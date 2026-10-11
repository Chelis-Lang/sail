#include <pthread.h>
#include <stdio.h>
#include <unistd.h>

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t changed = PTHREAD_COND_INITIALIZER;
static int ready;
static int stopped;

__attribute__((noinline)) static void *diagnostic_condition_wait_marker(void *unused) {
    (void)unused;
    pthread_mutex_lock(&lock);
    ready = 1;
    pthread_cond_broadcast(&changed);
    while (!stopped) pthread_cond_wait(&changed, &lock);
    pthread_mutex_unlock(&lock);
    return NULL;
}

__attribute__((noinline)) static int diagnostic_pipe_wait_marker(void) {
    char byte;
    return read(STDIN_FILENO, &byte, 1) == 1 ? 0 : 1;
}

int main(void) {
    pthread_t thread;
    if (pthread_create(&thread, NULL, diagnostic_condition_wait_marker, NULL)) return 2;
    pthread_mutex_lock(&lock);
    while (!ready) pthread_cond_wait(&changed, &lock);
    pthread_mutex_unlock(&lock);
    puts("READY");
    fflush(stdout);
    int result = diagnostic_pipe_wait_marker();
    pthread_mutex_lock(&lock);
    stopped = 1;
    pthread_cond_broadcast(&changed);
    pthread_mutex_unlock(&lock);
    if (pthread_join(thread, NULL)) return 3;
    return result;
}
