/* Correctness-only process-state fixture for sail-snap's conservative gates.
   No solver, benchmark or dependency on Z3 internals. */
#define _GNU_SOURCE
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

static atomic_int stopped;
static void *worker(void *unused) {
    (void)unused;
    while (!atomic_load(&stopped)) {
        struct timespec pause = {0, 1000000};
        nanosleep(&pause, NULL);
    }
    return NULL;
}

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    pthread_t thread;
    int threaded = strcmp(argv[1], "thread") == 0;
    if (threaded && pthread_create(&thread, NULL, worker, NULL) != 0) return 3;
    if (strcmp(argv[1], "timer") == 0) {
        signal(SIGALRM, SIG_IGN);
        struct itimerval timer = {{0, 0}, {30, 0}};
        if (setitimer(ITIMER_REAL, &timer, NULL) != 0) return 4;
    }
    if (strcmp(argv[1], "cpu") == 0) {
        struct rlimit cpu = {30, 30};
        if (setrlimit(RLIMIT_CPU, &cpu) != 0) return 5;
    }
    int descriptor = -1;
    if (strcmp(argv[1], "descriptor") == 0) {
        descriptor = open("/dev/null", O_RDONLY);
        if (descriptor < 0) return 6;
    }
#ifdef __linux__
    timer_t timer;
    int posix_timer = strcmp(argv[1], "posix-timer") == 0;
    if (posix_timer) {
        struct sigevent event = {.sigev_notify = SIGEV_NONE};
        if (timer_create(CLOCK_MONOTONIC, &event, &timer) != 0) return 7;
    }
#endif
    int byte;
    while ((byte = getc(stdin)) != EOF)
        if (putchar(byte) == EOF) return 8;
    if (ferror(stdin) || fflush(stdout) != 0) return 9;
    if (descriptor >= 0) close(descriptor);
    if (threaded) {
        atomic_store(&stopped, 1);
        if (pthread_join(thread, NULL) != 0) return 10;
    }
#ifdef __linux__
    if (posix_timer && timer_delete(timer) != 0) return 11;
#endif
    return 0;
}
