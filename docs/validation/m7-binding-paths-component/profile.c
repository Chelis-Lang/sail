#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/vfs.h>
#include <linux/magic.h>
#include <time.h>
#include <unistd.h>
#define FILES 327
#define WAVES 32
static char paths[FILES][4097];
static struct stat identities[FILES];
struct worker { int id; double old_ms[WAVES], descriptor_ms[WAVES]; int failed; };
static double now(void) { struct timespec t; if(clock_gettime(CLOCK_MONOTONIC,&t)) abort(); return t.tv_sec*1000.0+t.tv_nsec/1e6; }
static int old_match(int i) { char out[4097]; struct stat s; return realpath(paths[i],out) && !strcmp(out,paths[i]) && !stat(paths[i],&s) && s.st_dev==identities[i].st_dev && s.st_ino==identities[i].st_ino; }
static int new_match(int parent,int i) {
 int fd=open(paths[i],O_PATH|O_CLOEXEC); if(fd<0)return 0;
 struct stat s; char name[32], out[4097]; snprintf(name,sizeof(name),"%d",fd);
 ssize_t n=readlinkat(parent,name,out,sizeof(out));
 int yes=!fstat(fd,&s)&&s.st_dev==identities[i].st_dev&&s.st_ino==identities[i].st_ino&&n>=0&&n<(ssize_t)sizeof(out)&&n==(ssize_t)strlen(paths[i])&&!memcmp(out,paths[i],n);
 if(n>=10&&!memcmp(out+n-10," (deleted)",10))yes=0;
 close(fd); return yes;
}
static double sample(struct worker *w,int descriptor) {
 double start=now(); int parent=descriptor?open("/proc/self/fd",O_PATH|O_DIRECTORY|O_CLOEXEC):-1;
 if(descriptor&&parent<0){w->failed=1;return -1;}
 struct statfs proc;
 if(descriptor&&(fstatfs(parent,&proc)||proc.f_type!=PROC_SUPER_MAGIC)){close(parent);w->failed=1;return -1;}
 for(int i=0;i<FILES;i++)if(!(descriptor?new_match(parent,i):old_match(i)))w->failed=1;
 if(descriptor){close(parent);}
 return now()-start;
}
static void *run(void *p) {struct worker *w=p;for(int n=0;n<WAVES;n++){if((w->id+n)%2){w->descriptor_ms[n]=sample(w,1);w->old_ms[n]=sample(w,0);}else{w->old_ms[n]=sample(w,0);w->descriptor_ms[n]=sample(w,1);}}return NULL;}
int main(int argc,char **argv) {
 if(argc!=2){return 2;}
 char root[4097];
 if(!realpath(argv[1],root)){return 3;}
 for(int i=0;i<FILES;i++){if(snprintf(paths[i],sizeof(paths[i]),"%s/input-%03d",root,i)>=(int)sizeof(paths[i])||stat(paths[i],&identities[i]))return 4;}
 struct worker workers[4]={0};pthread_t threads[4];
 for(int i=0;i<4;i++){workers[i].id=i;if(pthread_create(&threads[i],NULL,run,&workers[i]))return 5;}
 for(int i=0;i<4;i++)if(pthread_join(threads[i],NULL)||workers[i].failed)return 6;
 puts("{\"scope\":\"Offline native syscall component; synthetic healthy paths, no live proof/currentness/latency acceptance\",\"files\":327,\"threads\":4,\"waves\":32,\"samples\":[");
 for(int i=0;i<4;i++)for(int n=0;n<WAVES;n++)printf("%s{\"thread\":%d,\"wave\":%d,\"canonical_stat_ms\":%.9f,\"descriptor_ms\":%.9f}",i||n?",\n":"",i,n,workers[i].old_ms[n],workers[i].descriptor_ms[n]);
 puts("\n]}");
 return 0;
}
