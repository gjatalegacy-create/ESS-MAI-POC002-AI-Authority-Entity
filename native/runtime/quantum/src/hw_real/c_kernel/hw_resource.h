/*
 * hw_resource.h — AIGovern-Cyber Hardware Layer
 * GJATA LEGACY™ — Quantum Platform
 */

#ifndef HW_RESOURCE_H
#define HW_RESOURCE_H


#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Kodi kthimi */
#define HW_OK              0
#define HW_ERR_NULL       -1
#define HW_ERR_READ       -2
#define HW_ERR_PARSE      -3
#define HW_ERR_INVARIANT  -4

/* Paths */
#define HW_CPU_SAMPLE_MS     100u
#define HW_CORES_PATH        "/sys/devices/system/cpu/online"
#define HW_CPU_STAT_PATH     "/proc/stat"
#define HW_MEMINFO_PATH      "/proc/meminfo"
#define HW_ENERGY_PATH_AC    "/sys/class/power_supply/AC/online"
#define HW_ENERGY_PATH_BAT   "/sys/class/power_supply/BAT0/capacity"

/*
 * HwSnapshot — gjendja e plotë e harduerit (ephemeral, 1 snapshot)
 * Invariantet:
 *   ram_free_bytes   ≤ ram_total_bytes
 *   cpu_load_pct     ∈ [0.0, 100.0]
 *   cores_active     ∈ [1, cores_total]
 *   energy_margin_pct∈ [0.0, 1.0]
 */
typedef struct {
    uint64_t ram_free_bytes;
    uint64_t ram_total_bytes;
    float    cpu_load_pct;
    uint32_t cores_active;
    uint32_t cores_total;
    float    energy_margin_pct;
    bool     on_battery;
    uint8_t  _pad[7];
    uint64_t ts_ns;
} HwSnapshot;

/* Pure Windows-counter calculation; kernel counters include idle time.
 * Samples must be monotonic, with a positive representable total delta and
 * idle delta <= total delta. Invalid input leaves load_pct unchanged. */
int hw_resource_cpu_load_from_counters(
    uint64_t idle1, uint64_t kern1, uint64_t user1,
    uint64_t idle2, uint64_t kern2, uint64_t user2, float *load_pct);

/* Pure Linux-counter calculation for the first eight /proc/stat fields.
 * Invalid/regressed/zero/overflowing intervals leave load_pct unchanged. */
int hw_resource_cpu_load_from_linux_counters(
    const uint64_t first[8], const uint64_t second[8], float *load_pct);

/* Public API */
int   hw_resource_snapshot(HwSnapshot *out);
int   hw_resource_validate(const HwSnapshot *snap);
float hw_resource_ram_free_ratio(const HwSnapshot *snap);
void  hw_resource_debug_print(const HwSnapshot *snap);

#ifdef __cplusplus
}
#endif

#endif /* HW_RESOURCE_H */
