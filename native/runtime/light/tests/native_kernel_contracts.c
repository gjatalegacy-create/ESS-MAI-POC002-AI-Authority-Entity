#define _POSIX_C_SOURCE 200809L
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <pthread.h>

static int clock_status = 0;
static struct timespec clock_value = {123, 456};
static int contract_clock_gettime(clockid_t id, struct timespec *ts) {
    (void)id;
    if (clock_status != 0) { return clock_status; }
    *ts = clock_value;
    return 0;
}
#define clock_gettime contract_clock_gettime
#include "shadow_gj_legacy.c"
#undef clock_gettime
#include "light_buss.c"

#define CHECK(expr) do { if (!(expr)) { \
    fprintf(stderr, "FAIL line %d: %s\n", __LINE__, #expr); return 1; \
} } while (0)

static LgcRequest sealed_request(void) {
    LgcRequest req = {0};
    req.flags = SGL_SEAL_PRIMITIVE ^ SGL_SEAL_XOR;
    memcpy(req.req_id, "kernel-contract", 15);
    memset(req.sha256, 'a', LGC_SHA_LEN - 1);
    return req;
}

static int clock_failure_contract(void) {
    LgcRequest req = sealed_request();
    LgcResult res;
    LgcStats stats;
    CHECK(lgc_init() == 0);
    CHECK(lgc_check(&req, &res) == 0);
    CHECK(res.decision == LGC_DECISION_PASS && res.ts == 123000000456ULL);
    clock_status = -1;
    memset(&res, 0xa5, sizeof(res));
    CHECK(lgc_check(&req, &res) == -2);
    CHECK(res.decision == LGC_DECISION_BLOCK && res.ts == 0);
    CHECK(strcmp((const char *)res.reason, "LGC_CLOCK_UNAVAILABLE") == 0);
    CHECK(memcmp(res.req_id, req.req_id, LGC_ID_LEN) == 0);
    CHECK(memcmp(res.sha256, req.sha256, LGC_SHA_LEN) == 0);
    clock_status = 0;
    clock_value.tv_nsec = 1000000000;
    CHECK(lgc_check(&req, &res) == -2 && res.decision == LGC_DECISION_BLOCK);
    clock_value.tv_nsec = -1;
    CHECK(lgc_check(&req, &res) == -2 && res.decision == LGC_DECISION_BLOCK);
    clock_value.tv_nsec = 0;
    clock_value.tv_sec = -1;
    CHECK(lgc_check(&req, &res) == -2 && res.decision == LGC_DECISION_BLOCK);
    clock_value.tv_sec = 0;
    CHECK(lgc_check(&req, &res) == -2 && res.decision == LGC_DECISION_BLOCK);
    clock_value.tv_sec = 123;
    CHECK(lgc_check(&req, &res) == 0 && res.decision == LGC_DECISION_PASS);
    req.flags = 0;
    CHECK(lgc_check(&req, &res) == 0 && res.decision == LGC_DECISION_BLOCK);
    lgc_get_stats(&stats);
    CHECK(stats.total == 8 && stats.passed == 2 && stats.blocked == 6);
    return 0;
}

static int bus_identity_contract(void) {
    char max_name[LB_MAX_MODULE];
    char longer[LB_MAX_MODULE + 1];
    LightBussMsg msg;
    LightBussStats stats;
    memset(max_name, 'A', LB_MAX_MODULE - 1);
    max_name[LB_MAX_MODULE - 1] = '\0';
    memcpy(longer, max_name, LB_MAX_MODULE - 1);
    longer[LB_MAX_MODULE - 1] = 'B';
    longer[LB_MAX_MODULE] = '\0';
    CHECK(light_buss_init() == LB_OK);
    CHECK(light_buss_send("LIGHT", max_name, (const uint8_t *)"x", 1, LB_PRIO_HIGH) == LB_OK);
    CHECK(light_buss_recv(longer, &msg) == LB_ERR_PARAM);
    CHECK(light_buss_recv(max_name, &msg) == LB_OK);
    CHECK(strcmp(msg.to, max_name) == 0 && msg.payload[0] == 'x');
    CHECK(light_buss_send("LIGHT", longer, NULL, 0, LB_PRIO_HIGH) == LB_ERR_PARAM);
    CHECK(light_buss_send(longer, "SHADOW", NULL, 0, LB_PRIO_HIGH) == LB_ERR_PARAM);
    CHECK(light_buss_send(NULL, "SHADOW", NULL, 0, LB_PRIO_HIGH) == LB_ERR_PARAM);
    CHECK(light_buss_send("", "SHADOW", NULL, 0, LB_PRIO_HIGH) == LB_ERR_PARAM);
    CHECK(light_buss_recv("", &msg) == LB_ERR_PARAM);
    CHECK(light_buss_send(max_name, "SHADOW", NULL, 0, LB_PRIO_LOW) == LB_OK);
    CHECK(light_buss_recv("SHADOW", &msg) == LB_OK);
    CHECK(strcmp(msg.from, max_name) == 0);
    light_buss_stats(&stats);
    CHECK(stats.total_sent == 2 && stats.total_recv == 2);
    light_buss_destroy();
    return 0;
}

static int repeated_init_contract(void) {
    LgcRequest req = sealed_request();
    LgcResult res;
    LgcStats before, after;
    CHECK(lgc_init() == 0);
    CHECK(lgc_check(&req, &res) == 0 && res.decision == LGC_DECISION_PASS);
    CHECK(lgc_freeze(7) == 0);
    lgc_get_stats(&before);
    CHECK(lgc_init() == 0);
    lgc_get_stats(&after);
    CHECK(after.frozen == 7);
    CHECK(after.total == before.total && after.passed == before.passed && after.blocked == before.blocked);
    CHECK(lgc_check(&req, &res) == 0 && res.decision == LGC_DECISION_BLOCK);
    CHECK(lgc_unfreeze() == 0);
    CHECK(lgc_check(&req, &res) == 0 && res.decision == LGC_DECISION_PASS);
    return 0;
}

static void *repeat_init(void *arg) {
    int i;
    (void)arg;
    for (i = 0; i < 10000; ++i) { if (lgc_init() != 0) { return (void *)1; } }
    return NULL;
}

static int concurrent_init_contract(void) {
    pthread_t threads[4];
    LgcStats before, after;
    void *result;
    int i;
    CHECK(lgc_init() == 0 && lgc_freeze(9) == 0);
    lgc_get_stats(&before);
    for (i = 0; i < 4; ++i) { CHECK(pthread_create(&threads[i], NULL, repeat_init, NULL) == 0); }
    for (i = 0; i < 4; ++i) { CHECK(pthread_join(threads[i], &result) == 0 && result == NULL); }
    lgc_get_stats(&after);
    CHECK(after.frozen == 9 && after.total == before.total);
    return 0;
}

int main(int argc, char **argv) {
    int status;
    if (argc != 2) { return 2; }
    if (strcmp(argv[1], "clock") == 0) { status = clock_failure_contract(); }
    else if (strcmp(argv[1], "bus") == 0) { status = bus_identity_contract(); }
    else if (strcmp(argv[1], "init") == 0) { status = repeated_init_contract(); }
    else if (strcmp(argv[1], "concurrent_init") == 0) { status = concurrent_init_contract(); }
    else { return 2; }
    if (status == 0) { printf("PASS %s\n", argv[1]); }
    return status;
}
