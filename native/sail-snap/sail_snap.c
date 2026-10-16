/*
 * sail-snap: experimental solver snapshots by fork; raw exactness unqualified.
 *
 * PROTOTYPE (lane M11-2, ADR 0051 draft). Loaded into the pinned, unmodified
 * Z3 with LD_PRELOAD (Linux) or DYLD_INSERT_LIBRARIES (macOS). Z3 reads its
 * `-smt2 -in` input one character at a time through `getc(stdin)` (libstdc++
 * `stdio_sync_filebuf`, libc++ `__stdinbuf`); this library interposes
 * `getc`/`ungetc` on stdin and nothing else.
 *
 * Markers. The client (sail-z3) may put `\0<name>\0` into the solver's input
 * at a point P. The library strips the marker, so Z3 receives exactly the
 * client's SMT-LIB bytes. When Z3 asks for the character at P, the library
 * forks. The parent carries on at once. The child is the snapshot: it holds
 * one inherited implementation-state image after Z3 has consumed the bytes
 * before P. Platform and solver atfork handlers may change that state: the
 * admission checks below reject known unsupported cases, not every possible
 * source of divergence. It detaches, binds the Unix socket
 * `$SAIL_SNAP_DIR/<name>` and parks.
 *
 * Resume. A client connects three times to the socket, each connection
 * starting with a fixed 29-byte header `SAILSNAP2 <role> <16 hex nonce>\n`,
 * roles `i` (stdin and stdout), `e` (stderr) and `s` (status). The parked
 * process forks again: the new child takes the connections as fds 0, 1 and 2,
 * drops the stdio read-ahead it inherited, and returns from `getc` with the
 * first byte of the new input, as if Z3 had just received it. The parked
 * process stays parked, so one snapshot serves any number of resumes. On the
 * status connection the parked process writes `ok <pid>\n` once the resumed
 * solver exists, and `exit <code>\n` when it exits.
 *
 * Fail-safe. Anything that goes wrong in the library drops a snapshot; it
 * never changes what Z3 reads. Parked snapshots exit after SAIL_SNAP_TTL_S
 * seconds idle, or when their socket file is removed or replaced (eviction).
 *
 * Environment (read once, at load):
 *   SAIL_SNAP_DIR_V2  directory for snapshot sockets; unset: inert (no
 *                     marker is ever expected)
 *   SAIL_SNAP_READY   file created at load, so the client knows markers will
 *                     be stripped before it sends any
 *   SAIL_SNAP_GAP_MS  take a snapshot only after this much time since the
 *                     process started, resumed or last snapshot (default 0)
 *   SAIL_SNAP_TTL_S   idle lifetime of a parked snapshot (default 1800)
 *   SAIL_SNAP_MAX     most snapshots one solver process takes (default 4096)
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/resource.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <sys/types.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#ifdef __linux__
#include <stdio_ext.h>
#else
#include <libproc.h>
#endif

#define NAME_MAX_LEN 96
#define HEADER_LEN 29
#define MAX_GROUPS 8
#define MAX_RUNNING 64

static int armed;
static char snap_dir[512];
static long gap_ms;
static long ttl_s = 1800;
static long max_snaps = 4096;
static long max_parked = 256;
static uint64_t max_bytes = 512ULL * 1024 * 1024;
static long taken;
static pid_t owned_snapshots[4096];
static size_t owned_count;
static int64_t last_mark_ms;
static int pushed = -1; /* one character of ungetc pushback on stdin */

static int (*real_getc)(FILE *);
static int (*real_ungetc)(int, FILE *);

static int64_t now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}

static long env_long(const char *name, long dflt) {
    const char *v = getenv(name);
    if (!v || !*v) return dflt;
    char *end;
    long n = strtol(v, &end, 10);
    return (*end || n < 0) ? dflt : n;
}

__attribute__((constructor)) static void sail_snap_init(void) {
#ifdef __linux__
    real_getc = (int (*)(FILE *))dlsym(RTLD_NEXT, "getc");
    real_ungetc = (int (*)(int, FILE *))dlsym(RTLD_NEXT, "ungetc");
#else
    real_getc = getc; /* not interposed inside the interposing image */
    real_ungetc = ungetc;
#endif
    const char *framing = getenv("SAIL_SNAP_FRAMING");
    const char *dir = getenv("SAIL_SNAP_DIR_V2");
    if (!framing || strcmp(framing, "2") != 0) return;
    if (!real_getc || !real_ungetc || !dir || !*dir || strlen(dir) >= sizeof snap_dir) return;
    strcpy(snap_dir, dir);
    gap_ms = env_long("SAIL_SNAP_GAP_MS", 0);
    ttl_s = env_long("SAIL_SNAP_TTL_S", 1800);
    max_snaps = env_long("SAIL_SNAP_MAX", 4096);
    max_parked = env_long("SAIL_SNAP_MAX_PARKED", 256);
    max_bytes = (uint64_t)env_long("SAIL_SNAP_MAX_BYTES", 512L * 1024 * 1024);
    last_mark_ms = now_ms();
    armed = 1;
    const char *ready = getenv("SAIL_SNAP_READY");
    if (ready && *ready) {
        int fd = open(ready, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
        if (fd >= 0) {
            const char signature[] = "sail-snap/3\n";
            (void)write(fd, signature, sizeof signature - 1);
            close(fd);
        }
    }
}

static int fork_kernel_state_supported(void);

/* ---- the parked snapshot ------------------------------------------------ */

static volatile sig_atomic_t got_child;
static void on_child(int sig) { (void)sig; got_child = 1; }

struct group { int used; char nonce[17]; int fd[3]; };     /* i, e, s */
struct running { pid_t pid; int status_fd; };

static void write_all(int fd, const char *s, size_t n) {
    while (n > 0) {
        ssize_t w = write(fd, s, n);
        if (w < 0 && errno == EINTR) continue;
        if (w <= 0) return;
        s += w; n -= (size_t)w;
    }
}

static int read_header(int fd, char *role, char nonce[17]) {
    char h[HEADER_LEN];
    size_t got = 0;
    struct timeval tv = {2, 0};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof tv);
    while (got < HEADER_LEN) {
        ssize_t r = read(fd, h + got, HEADER_LEN - got);
        if (r < 0 && errno == EINTR) continue;
        if (r <= 0) return -1;
        got += (size_t)r;
    }
    struct timeval zero = {0, 0};
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &zero, sizeof zero);
    if (memcmp(h, "SAILSNAP2 ", 10) != 0 || h[11] != ' ' || h[28] != '\n') return -1;
    if (h[10] != 'i' && h[10] != 'e' && h[10] != 's') return -1;
    *role = h[10];
    memcpy(nonce, h + 12, 16);
    nonce[16] = 0;
    return 0;
}

static int same_inode(const char *path, ino_t ino, dev_t dev) {
    struct stat st;
    return stat(path, &st) == 0 && st.st_ino == ino && st.st_dev == dev;
}

/* Saved signal dispositions of the solver, restored in a resumed child. */
static struct sigaction saved_int, saved_term, saved_hup, saved_pipe, saved_chld;

static void become_resumed(int listen_fd, struct group *g, struct group *groups,
                           struct running *run, int nrun) {
    close(listen_fd);
    for (int k = 0; k < MAX_GROUPS; k++)
        if (groups[k].used && &groups[k] != g)
            for (int r = 0; r < 3; r++) if (groups[k].fd[r] >= 0) close(groups[k].fd[r]);
    for (int k = 0; k < nrun; k++) close(run[k].status_fd);
    dup2(g->fd[0], 0);
    dup2(g->fd[0], 1);
    dup2(g->fd[1], 2);
    close(g->fd[0]);
    close(g->fd[1]);
    close(g->fd[2]);
    sigaction(SIGINT, &saved_int, NULL);
    sigaction(SIGTERM, &saved_term, NULL);
    sigaction(SIGHUP, &saved_hup, NULL);
    sigaction(SIGPIPE, &saved_pipe, NULL);
    sigaction(SIGCHLD, &saved_chld, NULL);
    /* Drop the stdio read-ahead inherited from the snapshot's parent: those
       bytes followed P in the old input. */
#ifdef __linux__
    __fpurge(stdin);
#else
    fpurge(stdin);
#endif
    clearerr(stdin);
    pushed = -1;
    taken = 0;
    last_mark_ms = now_ms();
}

/* Runs in the snapshot (the forked child). Returns only in a resumed solver. */
static void park(const char *name, int budget_fd, int ready_fd) {
    sigaction(SIGINT, NULL, &saved_int);
    sigaction(SIGTERM, NULL, &saved_term);
    sigaction(SIGHUP, NULL, &saved_hup);
    sigaction(SIGPIPE, NULL, &saved_pipe);
    sigaction(SIGCHLD, NULL, &saved_chld);
    setsid();
    signal(SIGINT, SIG_DFL);
    signal(SIGTERM, SIG_DFL);
    signal(SIGHUP, SIG_IGN);
    signal(SIGPIPE, SIG_IGN);
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_handler = on_child; /* no SA_RESTART: poll wakes on SIGCHLD */
    sigaction(SIGCHLD, &sa, NULL);

    /* Release every descriptor the solver held: the client waits for EOF on
       the old pipes. */
    int null = open("/dev/null", O_RDWR);
    if (null < 0) _exit(0);
    dup2(null, 0); dup2(null, 1); dup2(null, 2);
    for (int fd = 3; fd < 1024; fd++)
        if (fd != budget_fd && fd != ready_fd) close(fd);
    /* Output written before P has already been flushed by the parent. */
#ifdef __linux__
    __fpurge(stdout);
#else
    fpurge(stdout);
#endif

    char path[sizeof snap_dir + NAME_MAX_LEN + 2], tmp[sizeof path + 32];
    snprintf(path, sizeof path, "%s/%s", snap_dir, name);
    snprintf(tmp, sizeof tmp, "%s/.t%ld", snap_dir, (long)getpid());
    struct sockaddr_un addr;
    memset(&addr, 0, sizeof addr);
    addr.sun_family = AF_UNIX;
    if (strlen(tmp) >= sizeof addr.sun_path || strlen(path) >= sizeof addr.sun_path) _exit(0);
    strcpy(addr.sun_path, tmp);
    int lfd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (lfd < 0) _exit(0);
    unlink(tmp);
    if (bind(lfd, (struct sockaddr *)&addr, sizeof addr) != 0 || listen(lfd, 16) != 0) _exit(0);
    if (rename(tmp, path) != 0) { unlink(tmp); _exit(0); }
    struct stat st;
    if (stat(path, &st) != 0) _exit(0);
    /* `<name>.pid`, for memory accounting and tools. */
    char pidpath[sizeof path + 8];
    snprintf(pidpath, sizeof pidpath, "%s.pid", path);
    int pfd = open(pidpath, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
    if (pfd >= 0) {
        char pidtext[24];
        int pn = snprintf(pidtext, sizeof pidtext, "%ld\n", (long)getpid());
        write_all(pfd, pidtext, (size_t)pn);
        close(pfd);
    } else { unlink(path); _exit(0); }
    /* Publication and the pid file are serialized across every solver in
       this directory. The parent releases the lock only after this ack. */
    write_all(ready_fd, "1", 1);
    close(ready_fd);
    close(budget_fd);
    ino_t ino = st.st_ino;
    dev_t dev = st.st_dev;

    struct group groups[MAX_GROUPS];
    memset(groups, 0, sizeof groups);
    struct running run[MAX_RUNNING];
    int nrun = 0;
    int64_t last_use = now_ms();

    for (;;) {
        /* Reap resumed solvers and report their status. */
        int status;
        pid_t p;
        while ((p = waitpid(-1, &status, WNOHANG)) > 0) {
            for (int k = 0; k < nrun; k++) {
                if (run[k].pid != p) continue;
                int code = WIFEXITED(status) ? WEXITSTATUS(status)
                         : WIFSIGNALED(status) ? 128 + WTERMSIG(status) : 1;
                char msg[32];
                int n = snprintf(msg, sizeof msg, "exit %d\n", code);
                write_all(run[k].status_fd, msg, (size_t)n);
                close(run[k].status_fd);
                run[k] = run[--nrun];
                break;
            }
            last_use = now_ms();
        }
        got_child = 0;
        int evicted = !same_inode(path, ino, dev);
        int expired = now_ms() - last_use > (int64_t)ttl_s * 1000;
        if ((evicted || expired) && nrun == 0) {
            if (!evicted) {
                unlink(path);
            }
            _exit(0);
        }
        if (evicted || expired) {
            /* Stop accepting; wait for resumed solvers to finish. */
            if (lfd >= 0) { close(lfd); lfd = -1; }
            poll(NULL, 0, 50);
            continue;
        }
        struct pollfd pfd = {lfd, POLLIN, 0};
        int r = poll(&pfd, 1, 200);
        if (r <= 0 || !(pfd.revents & POLLIN)) continue;
        int c = accept(lfd, NULL, NULL);
        if (c < 0) continue;
        char role, nonce[17];
        if (read_header(c, &role, nonce) != 0) { close(c); continue; }
        int ri = role == 'i' ? 0 : role == 'e' ? 1 : 2;
        struct group *g = NULL;
        for (int k = 0; k < MAX_GROUPS && !g; k++)
            if (groups[k].used && strcmp(groups[k].nonce, nonce) == 0) g = &groups[k];
        for (int k = 0; k < MAX_GROUPS && !g; k++)
            if (!groups[k].used) {
                g = &groups[k];
                g->used = 1;
                memcpy(g->nonce, nonce, 17);
                g->fd[0] = g->fd[1] = g->fd[2] = -1;
            }
        if (!g || g->fd[ri] >= 0) { close(c); continue; }
        g->fd[ri] = c;
        if (g->fd[0] < 0 || g->fd[1] < 0 || g->fd[2] < 0) continue;
        last_use = now_ms();
        utimes(path, NULL); /* recency, for eviction by age */
        if (nrun == MAX_RUNNING) {
            for (int r2 = 0; r2 < 3; r2++) close(g->fd[r2]);
            g->used = 0;
            continue;
        }
        if (!fork_kernel_state_supported()) {
            for (int r2 = 0; r2 < 3; r2++) close(g->fd[r2]);
            g->used = 0;
            continue;
        }
        pid_t child = fork();
        if (child == 0) {
            become_resumed(lfd, g, groups, run, nrun);
            return; /* back into getc, now reading the new input */
        }
        if (child > 0) {
            /* Acknowledge: the client falls back to a fresh solver if this
               never arrives. */
            char ack[32];
            int an = snprintf(ack, sizeof ack, "ok %ld\n", (long)child);
            write_all(g->fd[2], ack, (size_t)an);
            run[nrun].pid = child;
            run[nrun].status_fd = g->fd[2];
            nrun++;
        } else {
            close(g->fd[2]);
        }
        close(g->fd[0]);
        close(g->fd[1]);
        g->used = 0;
    }
}

/* RSS is deliberately more conservative than PSS: each parked process is
   charged its entire resident image even when copy-on-write pages are shared.
   A one-MiB reserve covers the bounded socket/control bookkeeping. Unreadable
   process state refuses a fork, rather than treating it as zero memory. */
static int resident_bytes(pid_t pid, uint64_t *bytes) {
#ifdef __linux__
    char path[64];
    snprintf(path, sizeof path, "/proc/%ld/statm", (long)pid);
    FILE *f = fopen(path, "r");
    if (!f) return -1;
    unsigned long long size, pages;
    int ok = fscanf(f, "%llu %llu", &size, &pages) == 2;
    fclose(f);
    long page_size = sysconf(_SC_PAGESIZE);
    if (!ok || page_size <= 0 || pages > UINT64_MAX / (uint64_t)page_size) return -1;
    *bytes = pages * (uint64_t)page_size;
#else
    struct proc_taskinfo info;
    if (proc_pidinfo(pid, PROC_PIDTASKINFO, 0, &info, sizeof info) != (int)sizeof info) return -1;
    *bytes = info.pti_resident_size;
#endif
    return 0;
}

static int within_budget(void) {
    uint64_t proposed;
    const uint64_t reserve = 1024 * 1024;
    if (resident_bytes(getpid(), &proposed) != 0 || proposed > UINT64_MAX - reserve) return 0;
    proposed += reserve;
    long count = 0;
    DIR *d = opendir(snap_dir);
    if (!d) return 0;
    struct dirent *entry;
    int ok = 1;
    while ((entry = readdir(d))) {
        size_t n = strlen(entry->d_name);
        if (n < 5 || strcmp(entry->d_name + n - 4, ".pid") != 0) continue;
        char path[sizeof snap_dir + NAME_MAX_LEN + 8];
        if (snprintf(path, sizeof path, "%s/%s", snap_dir, entry->d_name) >= (int)sizeof path) { ok = 0; break; }
        FILE *f = fopen(path, "r");
        long value = 0;
        if (!f) { ok = 0; break; }
        int parsed = fscanf(f, "%ld", &value) == 1 && value > 0 && value <= INT32_MAX;
        fclose(f);
        if (!parsed) { ok = 0; break; }
        pid_t pid = (pid_t)value;
        if (kill(pid, 0) < 0 && errno == ESRCH) { unlink(path); continue; }
        uint64_t bytes;
        if (resident_bytes(pid, &bytes) != 0 || bytes > UINT64_MAX - reserve || proposed > UINT64_MAX - bytes - reserve) { ok = 0; break; }
        proposed += bytes + reserve;
        count++;
    }
    closedir(d);
    return ok && count < max_parked && proposed <= max_bytes;
}

/* A process image is not all of a solver's state. Other threads disappear
   in the child; pinned Z3 also finalizes idle timer threads in an atfork
   prepare callback, mutating the continuing parent. Kernel interval timers
   and consumed CPU budgets are not copied. These are conservative admission
   checks, not a proof of raw-output equality or a thread-creation lock. */
static int single_threaded(void) {
#ifdef __linux__
    DIR *tasks = opendir("/proc/self/task");
    if (!tasks) return 0;
    int count = 0;
    errno = 0;
    struct dirent *entry;
    while ((entry = readdir(tasks))) {
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) continue;
        count++;
    }
    int ok = errno == 0 && count == 1;
    if (closedir(tasks) != 0) ok = 0;
    return ok;
#else
    struct proc_taskinfo task;
    int n = proc_pidinfo(getpid(), PROC_PIDTASKINFO, 0, &task, sizeof task);
    return n == (int)sizeof task && task.pti_threadnum == 1;
#endif
}

static int fork_kernel_state_supported(void) {
    if (!single_threaded()) return 0;
    struct rlimit cpu;
    if (getrlimit(RLIMIT_CPU, &cpu) != 0 || cpu.rlim_cur != RLIM_INFINITY || cpu.rlim_max != RLIM_INFINITY) return 0;
    const int timers[] = {ITIMER_REAL, ITIMER_VIRTUAL, ITIMER_PROF};
    for (size_t i = 0; i < sizeof timers / sizeof timers[0]; i++) {
        struct itimerval timer;
        if (getitimer(timers[i], &timer) != 0 || timer.it_value.tv_sec || timer.it_value.tv_usec || timer.it_interval.tv_sec || timer.it_interval.tv_usec) return 0;
    }
    sigset_t pending;
    if (sigpending(&pending) != 0) return 0;
    for (int signum = 1; signum < NSIG; signum++)
        if (sigismember(&pending, signum) != 0) return 0;
#ifdef __linux__
    /* Linux exposes POSIX timers separately from getitimer(). Missing or
       unreadable procfs is unsupported, never assumed to mean no timers. */
    FILE *timers_file = fopen("/proc/self/timers", "r");
    if (!timers_file) return 0;
    int empty = fgetc(timers_file) == EOF && !ferror(timers_file);
    if (fclose(timers_file) != 0) empty = 0;
    if (!empty) return 0;
#endif
    return 1;
}

/* park() must release the original pipes to let their client see EOF. It
   cannot silently discard a solver-owned proof/trace/output descriptor. */
static int only_standard_descriptors(void) {
#ifdef __linux__
    DIR *files = opendir("/proc/self/fd");
    if (!files) return 0;
    int own = dirfd(files), ok = 1;
    struct dirent *entry;
    errno = 0;
    while ((entry = readdir(files))) {
        if (strcmp(entry->d_name, ".") == 0 || strcmp(entry->d_name, "..") == 0) continue;
        char *end;
        long fd = strtol(entry->d_name, &end, 10);
        if (*end || fd < 0 || (fd > 2 && fd != own)) { ok = 0; break; }
    }
    if (errno != 0) ok = 0;
    if (closedir(files) != 0) ok = 0;
    return ok;
#else
    struct proc_fdinfo files[128];
    int n = proc_pidinfo(getpid(), PROC_PIDLISTFDS, 0, files, sizeof files);
    if (n <= 0 || (size_t)n >= sizeof files || (size_t)n % sizeof files[0]) return 0;
    for (size_t i = 0; i < (size_t)n / sizeof files[0]; i++)
        if (files[i].proc_fd > 2) return 0;
    return 1;
#endif
}

static void snapshot(const char *name) {
    if (taken >= max_snaps || !fork_kernel_state_supported() || !only_standard_descriptors()) return;
    for (size_t i = 0; i < owned_count;) {
        pid_t got = waitpid(owned_snapshots[i], NULL, WNOHANG);
        if (got > 0 || (got < 0 && errno == ECHILD)) owned_snapshots[i] = owned_snapshots[--owned_count];
        else i++;
    }
    if (owned_count == sizeof owned_snapshots / sizeof owned_snapshots[0]) return;
    int64_t now = now_ms();
    if (gap_ms > 0 && now - last_mark_ms < gap_ms) return;
    char lock_path[sizeof snap_dir + 32];
    snprintf(lock_path, sizeof lock_path, "%s/.budget-lock", snap_dir);
    int lock_fd = open(lock_path, O_RDWR | O_CREAT | O_CLOEXEC, 0600);
    if (lock_fd < 0) return;
    if (flock(lock_fd, LOCK_EX | LOCK_NB) != 0) { close(lock_fd); return; }
    /* Never overwrite a live generation's pid record, even after eviction.
       It remains charged until its process actually exits. */
    char pid_path[sizeof snap_dir + NAME_MAX_LEN + 8];
    snprintf(pid_path, sizeof pid_path, "%s/%s.pid", snap_dir, name);
    FILE *existing = fopen(pid_path, "r");
    if (existing) {
        long pid = 0;
        int parsed = fscanf(existing, "%ld", &pid) == 1 && pid > 0 && pid <= INT32_MAX;
        fclose(existing);
        if (!parsed || kill((pid_t)pid, 0) == 0 || errno != ESRCH) { close(lock_fd); return; }
    } else if (errno != ENOENT) { close(lock_fd); return; }
    if (!within_budget()) { close(lock_fd); return; }
    int ready[2];
    if (pipe(ready) != 0) { close(lock_fd); return; }
    fflush(stdout);
    pid_t pid = fork();
    if (pid < 0) { close(ready[0]); close(ready[1]); close(lock_fd); return; }
    if (pid > 0) {
        close(ready[1]);
        struct pollfd pfd = {ready[0], POLLIN, 0};
        char ack;
        int published = poll(&pfd, 1, 2000) > 0 && read(ready[0], &ack, 1) == 1 && ack == '1';
        close(ready[0]);
        if (!published) { kill(pid, SIGKILL); while (waitpid(pid, NULL, 0) < 0 && errno == EINTR) {} }
        else owned_snapshots[owned_count++] = pid;
        flock(lock_fd, LOCK_UN);
        close(lock_fd);
        taken++;
        last_mark_ms = now;
        return;
    }
    close(ready[0]);
    park(name, lock_fd, ready[1]); /* returns in a resumed solver */
}

/* ---- interposed stdin reads ---------------------------------------------- */

static int valid_name_char(int c) {
    return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'z') || c == '-' || c == '_';
}

static int sail_getc(FILE *f) {
    if (f != stdin || !armed) return real_getc(f);
    if (pushed >= 0) {
        int c = pushed;
        pushed = -1;
        return c;
    }
    for (;;) {
        int c = real_getc(f);
        if (c != 0) return c;
        /* A marker: `\0<name>\0`. */
        char name[NAME_MAX_LEN + 1];
        size_t n = 0;
        int ok = 1;
        for (;;) {
            int d = real_getc(f);
            if (d == EOF) return EOF;
            if (d == 0) break;
            if (n < NAME_MAX_LEN && valid_name_char(d)) name[n++] = (char)d;
            else ok = 0;
        }
        name[n] = 0;
        if (n == 0 && ok) return 0; /* escaped data NUL */
        if (ok && n > 0) snapshot(name);
    }
}

static int sail_ungetc(int c, FILE *f) {
    if (f != stdin || !armed) return real_ungetc(c, f);
    if (c == EOF || pushed >= 0) return EOF;
    pushed = (unsigned char)c;
    return pushed;
}

#ifdef __linux__
int getc(FILE *f) { return sail_getc(f); }
int _IO_getc(FILE *f) { return sail_getc(f); }
int fgetc(FILE *f) { return sail_getc(f); }
int ungetc(int c, FILE *f) { return sail_ungetc(c, f); }
#else
__attribute__((used)) static struct { const void *r; const void *o; } sail_interpose[]
    __attribute__((section("__DATA,__interpose"))) = {
        {(const void *)sail_getc, (const void *)getc},
        {(const void *)sail_getc, (const void *)fgetc},
        {(const void *)sail_ungetc, (const void *)ungetc},
};
#endif
