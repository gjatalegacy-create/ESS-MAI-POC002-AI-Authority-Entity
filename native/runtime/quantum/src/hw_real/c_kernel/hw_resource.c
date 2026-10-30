/*
 * hw_resource.c — AIGovern-Cyber Hardware Layer
 * GJATA LEGACY™ — Quantum Platform
 */

/* POSIX feature macro — PARA çdo include (vetëm në POSIX) */
#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L
#endif

#include "hw_resource.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdbool.h>
#include <errno.h>
#include <time.h>
#include <math.h>

#ifdef _WIN32
/* Windows: API reale të sistemit — RAM/CPU/cores/energji lexohen NATIVE.
 * Kompilohet me gcc (mingw-w64); winpthreads mbulon shtresën POSIX të kohës. */
#include <windows.h>
#else
#include <unistd.h>
#include <sys/sysinfo.h>
#endif

/* ── CPU raw stats — unsigned long long për %llu (vetëm POSIX /proc/stat) ── */
#ifndef _WIN32
typedef struct {
    unsigned long long user;
    unsigned long long nice;
    unsigned long long system;
    unsigned long long idle;
    unsigned long long iowait;
    unsigned long long irq;
    unsigned long long softirq;
    unsigned long long steal;
} CpuRawStat;
#endif

/* ── timestamp monotonic ns ─────────────────────────────────────────────── */
#ifdef _WIN32
static uint64_t monotonic_ns(void) {
    LARGE_INTEGER freq, now;
    if (!QueryPerformanceFrequency(&freq) || freq.QuadPart <= 0) return 0ULL;
    if (!QueryPerformanceCounter(&now))                          return 0ULL;
    /* ns = ticks * 1e9 / freq — ndarje e para shumëzimit kundër overflow */
    uint64_t sec  = (uint64_t)now.QuadPart / (uint64_t)freq.QuadPart;
    uint64_t rem  = (uint64_t)now.QuadPart % (uint64_t)freq.QuadPart;
    return sec * 1000000000ULL + rem * 1000000000ULL / (uint64_t)freq.QuadPart;
}
#else
static uint64_t monotonic_ns(void) {
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts) != 0) {
        return 0ULL;
    }
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}
#endif

/* ── fle microsekonda ───────────────────────────────────────────────────── */
#ifdef _WIN32
static void sleep_us(unsigned int us) {
    /* Sleep() punon në ms; rrumbullakos lart, minimumi 1ms për mostrim CPU */
    unsigned int ms = (us + 999u) / 1000u;
    Sleep((ms > 0u) ? ms : 1u);
}
#else
static void sleep_us(unsigned int us) {
    struct timespec ts;
    ts.tv_sec  = (time_t)(us / 1000000u);
    ts.tv_nsec = (long)((us % 1000000u) * 1000u);
    nanosleep(&ts, NULL);
}
#endif

/* ═════════════════════════════════════════════════════════════════════════
 * §1  RAM — Windows: GlobalMemoryStatusEx (native) | POSIX: sysinfo() syscall
 * ═════════════════════════════════════════════════════════════════════════ */
#ifdef _WIN32
static int read_ram(uint64_t *free_bytes, uint64_t *total_bytes) {
    if (!free_bytes || !total_bytes) return HW_ERR_NULL;

    MEMORYSTATUSEX ms;
    memset(&ms, 0, sizeof(ms));
    ms.dwLength = sizeof(ms);
    if (!GlobalMemoryStatusEx(&ms)) return HW_ERR_READ;

    *total_bytes = (uint64_t)ms.ullTotalPhys;
    *free_bytes  = (uint64_t)ms.ullAvailPhys;
    if (*total_bytes == 0ULL) return HW_ERR_PARSE;
    return HW_OK;
}
#else
static int read_ram(uint64_t *free_bytes, uint64_t *total_bytes) {
    if (!free_bytes || !total_bytes) return HW_ERR_NULL;

    struct sysinfo si;
    if (sysinfo(&si) == 0) {
        *total_bytes = (uint64_t)si.totalram  * (uint64_t)si.mem_unit;
        *free_bytes  = (uint64_t)si.freeram   * (uint64_t)si.mem_unit
                     + (uint64_t)si.bufferram * (uint64_t)si.mem_unit;
        return HW_OK;
    }

    /* fallback: /proc/meminfo */
    FILE *f = fopen(HW_MEMINFO_PATH, "r");
    if (!f) return HW_ERR_READ;

    char line[128];
    uint64_t mem_total = 0, mem_avail = 0, mem_free = 0;
    while (fgets(line, (int)sizeof(line), f)) {
        unsigned long val = 0;
        if (sscanf(line, "MemTotal: %lu kB", &val) == 1)
            mem_total = (uint64_t)val * 1024ULL;
        else if (sscanf(line, "MemAvailable: %lu kB", &val) == 1)
            mem_avail = (uint64_t)val * 1024ULL;
        else if (sscanf(line, "MemFree: %lu kB", &val) == 1)
            mem_free  = (uint64_t)val * 1024ULL;
    }
    fclose(f);

    if (mem_total == 0) return HW_ERR_PARSE;
    *total_bytes = mem_total;
    *free_bytes  = (mem_avail > 0) ? mem_avail : mem_free;
    return HW_OK;
}
#endif /* _WIN32 read_ram */

/* ═════════════════════════════════════════════════════════════════════════
 * §2  CPU LOAD — Windows: dy lexime GetSystemTimes me 100ms ndërmjet
 *              | POSIX:   dy lexime /proc/stat me 100ms ndërmjet
 *
 * Formula (e njëjtë në të dyja):
 *   active = total - idle
 *   load%  = (active2 - active1) / (total2 - total1) × 100.0
 * ═════════════════════════════════════════════════════════════════════════ */
int hw_resource_cpu_load_from_counters(
    uint64_t idle1, uint64_t kern1, uint64_t user1,
    uint64_t idle2, uint64_t kern2, uint64_t user2,
    float *load_pct)
{
    if (!load_pct) return HW_ERR_NULL;

    /* Kernel time includes idle. Validate before unsigned subtraction;
     * an unavailable/inconsistent interval is not a measured CPU load. */
    if (idle2 < idle1 || kern2 < kern1 || user2 < user1)
        return HW_ERR_READ;
    uint64_t didle = idle2 - idle1;
    uint64_t dkern = kern2 - kern1;
    uint64_t duser = user2 - user1;
    if (dkern > UINT64_MAX - duser) return HW_ERR_READ;
    uint64_t dtotal = dkern + duser;
    if (dtotal == 0ULL || didle > dkern || didle > dtotal) return HW_ERR_READ;

    float load = 100.0f * (float)(dtotal - didle) / (float)dtotal;
    if (load < 0.0f)   load = 0.0f;
    if (load > 100.0f) load = 100.0f;
    *load_pct = load;
    return HW_OK;
}


/* /proc/stat fields: user, nice, system, idle, iowait, irq, softirq, steal.
 * D = sum(delta[i]); I = delta[idle] + delta[iowait]. A load observation
 * exists only when every delta is monotonic and 0 < D <= UINT64_MAX. */
int hw_resource_cpu_load_from_linux_counters(
    const uint64_t first[8], const uint64_t second[8], float *load_pct)
{
    if (!first || !second || !load_pct) return HW_ERR_NULL;
    uint64_t total = 0ULL, idle = 0ULL;
    for (unsigned int i = 0; i < 8u; ++i) {
        if (second[i] < first[i]) return HW_ERR_READ;
        uint64_t delta = second[i] - first[i];
        if (delta > UINT64_MAX - total) return HW_ERR_READ;
        total += delta;
        if (i == 3u || i == 4u) idle += delta;
    }
    if (total == 0ULL) return HW_ERR_READ;
    *load_pct = (float)(100.0 * ((double)(total - idle) / (double)total));
    return HW_OK;
}

#ifdef _WIN32
static uint64_t ft_u64(const FILETIME *ft) {
    ULARGE_INTEGER u;
    u.LowPart  = ft->dwLowDateTime;
    u.HighPart = ft->dwHighDateTime;
    return (uint64_t)u.QuadPart;
}

static int read_cpu_load(float *load_pct) {
    if (!load_pct) return HW_ERR_NULL;

    FILETIME idle1, kern1, user1, idle2, kern2, user2;
    if (!GetSystemTimes(&idle1, &kern1, &user1)) return HW_ERR_READ;
    sleep_us(HW_CPU_SAMPLE_MS * 1000u);
    if (!GetSystemTimes(&idle2, &kern2, &user2)) return HW_ERR_READ;

    uint64_t i1 = ft_u64(&idle1), k1 = ft_u64(&kern1), u1 = ft_u64(&user1);
    uint64_t i2 = ft_u64(&idle2), k2 = ft_u64(&kern2), u2 = ft_u64(&user2);
    int rc = hw_resource_cpu_load_from_counters(i1, k1, u1, i2, k2, u2, load_pct);
    if (rc != HW_OK) {
        fprintf(stderr,
            "[HW_CPU_SAMPLE_INVALID] rc=%d idle1=%llu kernel1=%llu user1=%llu"
            " idle2=%llu kernel2=%llu user2=%llu\n",
            rc, (unsigned long long)i1, (unsigned long long)k1,
            (unsigned long long)u1, (unsigned long long)i2,
            (unsigned long long)k2, (unsigned long long)u2);
    }
    return rc;
}
#else
static int read_cpu_raw(CpuRawStat *s) {
    if (!s) return HW_ERR_NULL;
    FILE *f = fopen(HW_CPU_STAT_PATH, "r");
    if (!f) return HW_ERR_READ;

    char tag[8];
    /* %llu — unsigned long long, saktë me CpuRawStat */
    int n = fscanf(f, "%7s %llu %llu %llu %llu %llu %llu %llu %llu",
                   tag,
                   &s->user, &s->nice, &s->system, &s->idle,
                   &s->iowait, &s->irq, &s->softirq, &s->steal);
    fclose(f);
    return (n == 9) ? HW_OK : HW_ERR_PARSE;
}

static int compute_cpu_load(const CpuRawStat *s1, const CpuRawStat *s2,
                            float *load_pct) {
    const uint64_t first[8] = {
        s1->user, s1->nice, s1->system, s1->idle,
        s1->iowait, s1->irq, s1->softirq, s1->steal
    };
    const uint64_t second[8] = {
        s2->user, s2->nice, s2->system, s2->idle,
        s2->iowait, s2->irq, s2->softirq, s2->steal
    };
    return hw_resource_cpu_load_from_linux_counters(first, second, load_pct);
}

static int read_cpu_load(float *load_pct) {
    if (!load_pct) return HW_ERR_NULL;
    CpuRawStat s1, s2;
    memset(&s1, 0, sizeof(s1));
    memset(&s2, 0, sizeof(s2));
    if (read_cpu_raw(&s1) != HW_OK) return HW_ERR_READ;
    sleep_us(HW_CPU_SAMPLE_MS * 1000u);
    if (read_cpu_raw(&s2) != HW_OK) return HW_ERR_READ;
    int rc = compute_cpu_load(&s1, &s2, load_pct);
    if (rc != HW_OK)
        fprintf(stderr, "[HW_CPU_SAMPLE_INVALID] platform=POSIX rc=%d\n", rc);
    return rc;
}
#endif /* _WIN32 cpu load */

/* ═════════════════════════════════════════════════════════════════════════
 * §3  CORES — Windows: GetSystemInfo | POSIX: /sys/devices/system/cpu/online
 * ═════════════════════════════════════════════════════════════════════════ */
#ifdef _WIN32
static int read_cores(uint32_t *active, uint32_t *total) {
    if (!active || !total) return HW_ERR_NULL;

    SYSTEM_INFO si;
    memset(&si, 0, sizeof(si));
    GetSystemInfo(&si);

    uint32_t n = (uint32_t)si.dwNumberOfProcessors;
    *active = (n > 0u) ? n : 1u;
    *total  = *active;
    return HW_OK;
}
#else
/* Formati: "0-7" ose "0,2,4-6" */
static uint32_t parse_cpu_list(const char *buf) {
    if (!buf || *buf == '\0') return 1u;
    uint32_t count = 0u;
    const char *p  = buf;

    while (*p && *p != '\n' && *p != '\r') {
        if (*p < '0' || *p > '9') { p++; continue; }

        uint32_t start = 0u;
        while (*p >= '0' && *p <= '9') {
            start = start * 10u + (uint32_t)(*p - '0');
            p++;
        }
        if (*p == '-') {
            p++;
            uint32_t end = 0u;
            while (*p >= '0' && *p <= '9') {
                end = end * 10u + (uint32_t)(*p - '0');
                p++;
            }
            if (end >= start) count += end - start + 1u;
        } else {
            count += 1u;
        }
        if (*p == ',') p++;
    }
    return (count > 0u) ? count : 1u;
}

static int read_cores(uint32_t *active, uint32_t *total) {
    if (!active || !total) return HW_ERR_NULL;

    FILE *f = fopen(HW_CORES_PATH, "r");
    if (f) {
        char buf[256];
        memset(buf, 0, sizeof(buf));
        if (fgets(buf, (int)sizeof(buf) - 1, f))
            *active = parse_cpu_list(buf);
        else
            *active = 1u;
        fclose(f);
    } else {
        long n = sysconf(_SC_NPROCESSORS_ONLN);
        *active = (n > 0L) ? (uint32_t)n : 1u;
    }

    long n_conf = sysconf(_SC_NPROCESSORS_CONF);
    *total = (n_conf > 0L) ? (uint32_t)n_conf : *active;

    if (*active > *total) *active = *total;
    if (*total  == 0u)    *total  = 1u;
    if (*active == 0u)    *active = 1u;
    return HW_OK;
}
#endif /* _WIN32 cores */

/* ═════════════════════════════════════════════════════════════════════════
 * §4  ENERGY MARGIN — Windows: GetSystemPowerStatus | POSIX: sysfs AC/BAT
 * ═════════════════════════════════════════════════════════════════════════ */
typedef struct {
    const char *source;
    const char *cause;
    bool physical_observation;
} EnergyEvidence;

static void trace_energy_evidence(const HwSnapshot *snap, const EnergyEvidence *evidence) {
    uint32_t cpu_bits = 0u, energy_bits = 0u;
    memcpy(&cpu_bits, &snap->cpu_load_pct, sizeof(cpu_bits));
    memcpy(&energy_bits, &snap->energy_margin_pct, sizeof(energy_bits));
    /* This same-capture evidence does not change admission or grant authority. */
    fprintf(stderr,
        "[HW_ENERGY] ts_ns=%llu source=%s cause=%s physical_observation=%u"
        " cpu_bits=%08x energy_bits=%08x on_battery=%u effect=UNPROVEN\n",
        (unsigned long long)snap->ts_ns, evidence->source, evidence->cause,
        (unsigned int)evidence->physical_observation, (unsigned int)cpu_bits,
        (unsigned int)energy_bits, (unsigned int)snap->on_battery);
}

#ifdef _WIN32
static int read_energy_margin(float *margin, bool *on_battery, float cpu_load,
                              EnergyEvidence *evidence) {
    if (!margin || !on_battery) return HW_ERR_NULL;

    SYSTEM_POWER_STATUS sps;
    memset(&sps, 0, sizeof(sps));
    int power_read_ok = GetSystemPowerStatus(&sps) != 0;
    if (power_read_ok) {
        /* ACLineStatus: 1 = AC, 0 = bateri, 255 = e panjohur */
        if (sps.ACLineStatus == 1) {
            *evidence = (EnergyEvidence){"WINDOWS_AC", "ac_online", true};
            *on_battery = false;
            *margin     = 1.0f;
            return HW_OK;
        }
        if (sps.ACLineStatus == 0) {
            *evidence = (EnergyEvidence){
                (sps.BatteryLifePercent <= 100) ? "WINDOWS_BATTERY" : "BATTERY_ESTIMATE",
                (sps.BatteryLifePercent <= 100) ? "battery_percent" : "battery_percent_unknown",
                sps.BatteryLifePercent <= 100};
            *on_battery = true;
            /* BatteryLifePercent: 0..100, 255 = e panjohur */
            float pct = (sps.BatteryLifePercent <= 100)
                      ? (float)sps.BatteryLifePercent / 100.0f
                      : 0.50f;
            if (pct < 0.0f) pct = 0.0f;
            if (pct > 1.0f) pct = 1.0f;
            *margin = pct;
            return HW_OK;
        }
    }

    /* e panjohur → estimate nga CPU load (i njëjti model si POSIX) */
    *evidence = (EnergyEvidence){"CPU_ESTIMATE",
        power_read_ok ? "ac_status_unknown" : "power_api_failed", false};
    *on_battery = false;
    *margin     = 1.0f - (cpu_load / 100.0f) * 0.30f;
    if (*margin < 0.0f) *margin = 0.0f;
    if (*margin > 1.0f) *margin = 1.0f;
    return HW_OK;
}
#else
static int read_energy_margin(float *margin, bool *on_battery, float cpu_load,
                              EnergyEvidence *evidence) {
    if (!margin || !on_battery) return HW_ERR_NULL;
    *on_battery = false;

    FILE *f = fopen(HW_ENERGY_PATH_AC, "r");
    if (f) {
        int ac_online = 0;
        if (fscanf(f, "%d", &ac_online) == 1 && ac_online == 1) {
            *evidence = (EnergyEvidence){"SYSFS_AC", "ac_online", true};
            fclose(f);
            *margin     = 1.0f;
            *on_battery = false;
            return HW_OK;
        }
        fclose(f);
    }

    f = fopen(HW_ENERGY_PATH_BAT, "r");
    if (f) {
        int capacity = 0;
        if (fscanf(f, "%d", &capacity) == 1) {
            *evidence = (EnergyEvidence){
                (capacity >= 0 && capacity <= 100) ? "SYSFS_BATTERY" : "BATTERY_CLAMPED",
                (capacity >= 0 && capacity <= 100) ? "battery_percent" : "battery_percent_invalid",
                capacity >= 0 && capacity <= 100};
            fclose(f);
            *on_battery = true;
            *margin     = (float)capacity / 100.0f;
            if (*margin < 0.0f) *margin = 0.0f;
            if (*margin > 1.0f) *margin = 1.0f;
            return HW_OK;
        }
        fclose(f);
    }

    /* estimate nga CPU load */
    *evidence = (EnergyEvidence){"CPU_ESTIMATE", "sysfs_power_unavailable", false};
    *on_battery = false;
    *margin     = 1.0f - (cpu_load / 100.0f) * 0.30f;
    if (*margin < 0.0f) *margin = 0.0f;
    if (*margin > 1.0f) *margin = 1.0f;
    return HW_OK;
}
#endif /* _WIN32 energy */

/* ═════════════════════════════════════════════════════════════════════════
 * §5  PUBLIC API
 * ═════════════════════════════════════════════════════════════════════════ */
int hw_resource_snapshot(HwSnapshot *out) {
    if (!out) return HW_ERR_NULL;
    memset(out, 0, sizeof(HwSnapshot));

    if (read_ram(&out->ram_free_bytes, &out->ram_total_bytes) != HW_OK)
        return HW_ERR_READ;

    if (read_cpu_load(&out->cpu_load_pct) != HW_OK)
        return HW_ERR_READ;

    if (read_cores(&out->cores_active, &out->cores_total) != HW_OK) {
        out->cores_active = 1u;
        out->cores_total  = 1u;
    }

    EnergyEvidence energy_evidence = {"UNAVAILABLE", "energy_reader_error", false};
    if (read_energy_margin(&out->energy_margin_pct,
                           &out->on_battery,
                           out->cpu_load_pct, &energy_evidence) != HW_OK) {
        energy_evidence = (EnergyEvidence){"DEFAULT_ESTIMATE", "energy_reader_error", false};
        out->energy_margin_pct = 1.0f;
        out->on_battery        = false;
    }

    out->ts_ns = monotonic_ns();
    trace_energy_evidence(out, &energy_evidence);

    if (!hw_resource_validate(out)) return HW_ERR_INVARIANT;
    return HW_OK;
}

int hw_resource_validate(const HwSnapshot *snap) {
    if (!snap)                                        return 0;
    if (snap->ram_total_bytes == 0u)                  return 0;
    if (snap->ram_free_bytes > snap->ram_total_bytes) return 0;
    if (!isfinite(snap->cpu_load_pct))              return 0;
    if (snap->cpu_load_pct < 0.0f)                   return 0;
    if (snap->cpu_load_pct > 100.0f)                 return 0;
    if (snap->cores_active  == 0u)                   return 0;
    if (snap->cores_total   == 0u)                   return 0;
    if (snap->cores_active  > snap->cores_total)     return 0;
    if (!isfinite(snap->energy_margin_pct))         return 0;
    if (snap->energy_margin_pct < 0.0f)              return 0;
    if (snap->energy_margin_pct > 1.0f)              return 0;
    return 1;
}

float hw_resource_ram_free_ratio(const HwSnapshot *snap) {
    if (!snap || snap->ram_total_bytes == 0u) return 0.0f;
    float r = (float)snap->ram_free_bytes / (float)snap->ram_total_bytes;
    if (r < 0.0f) return 0.0f;
    if (r > 1.0f) return 1.0f;
    return r;
}

void hw_resource_debug_print(const HwSnapshot *snap) {
    if (!snap) { fprintf(stderr, "[HW_RESOURCE] snap=NULL\n"); return; }
    fprintf(stderr,
        "[HW_RESOURCE] ram_free=%llu MB / total=%llu MB"
        " | cpu=%.1f%% | cores=%u/%u | energy=%.2f (%s)\n",
        (unsigned long long)(snap->ram_free_bytes  / (1024ULL*1024ULL)),
        (unsigned long long)(snap->ram_total_bytes / (1024ULL*1024ULL)),
        snap->cpu_load_pct,
        snap->cores_active, snap->cores_total,
        snap->energy_margin_pct,
        snap->on_battery ? "BAT" : "AC");
}
