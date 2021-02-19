#define COUNT_BYTES

#define _GNU_SOURCE


#include <stdatomic.h>
#include <stdint.h>
#include <dlfcn.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <threads.h>
#include <unistd.h>
#include <execinfo.h>
#include <sys/types.h>


const int ALLOC_LIMIT = 100;

atomic_size_t mem_allocated_total_bytes = 0;
thread_local FILE * file = 0;
thread_local unsigned long long mem_allocated_cnt = 0;
thread_local unsigned long long mem_allocated_bytes = 0;
thread_local size_t tid = 0;
thread_local int getting_trace = 0;
thread_local int last_size = 0;

#define MAGIC 0x12345678991301

struct Header {
    uint64_t magic;
    uint64_t size;
    uint64_t tid;
    uint64_t func;
};


char tmpbuff[1024];
unsigned long tmppos = 0;
unsigned long tmpallocs = 0;

void *memset(void*,int,size_t);
void *memmove(void *to, const void *from, size_t size);

/*=========================================================
 * interception points
 */

extern void * __libc_calloc(size_t nmemb, size_t size);
extern void * __libc_malloc(size_t size);
extern void   __libc_free(void *ptr);
extern void * __libc_realloc(void *ptr, size_t size);
extern void * __libc_memalign(size_t blocksize, size_t bytes);

static void *(*myfn_mmap)(void *addr, size_t length, int prot, int flags, int fd, off_t offset);
static int (*myfn_posix_memalign)(void **memptr, size_t alignment, size_t size);
static int (*myfn_munmap)(void *addr, size_t len);





static void thread_init() {
    if (file == NULL) {
	if (tid == 0) tid = gettid();
	char buf[1000];
	sprintf(buf, "/tmp/dump/logs/%ld", tid);
	file = fopen(buf, "w");
	fprintf(file, "initializing tid: %ld\n", tid);
    }
}


#define ALIGN sizeof(struct Header)


thread_local int printing_trace = 0;

/*
 * O: 0x7fc4baef3618
O: 0x7fc4baef38cc
O: 0x560023dba768
--
O: 0x5600244dacc9
O: 0x560023009788
O: 0x7fc4baaed0b3
*/
/*
void print_trace2(void) {
    if (printing_trace) return;
    printing_trace = 1;
    char **strings;
    size_t i, size;
    enum Constexpr { MAX_SIZE = 1024 };
    void *array[MAX_SIZE];
    fprintf(stderr, "S0\n");
    size = backtrace(array, MAX_SIZE);
    fprintf(stderr, "S1\n");
    strings = backtrace_symbols(array, size);
    puts("XXYY\n");
    for (i = 0; i < size; i++)
        fprintf(stderr, "O: %s\n", strings[i]);
    puts("YY\n");
    free(strings);
    printing_trace = 0;
}
*/

void * get_trace(int64_t alloc_size) {
    if (alloc_size < ALLOC_LIMIT && (rand() % 100 != 0 && last_size == alloc_size)) {
    // if (alloc_size != 128 && alloc_size < ALLOC_LIMIT && (rand() % 100 != 0)) {
        return (void *)1;
    }
    last_size = alloc_size;

    if (getting_trace) return 0;

    getting_trace = 1;
    enum Constexpr { MAX_SIZE = 10 };
    void *array[MAX_SIZE];
    size_t i, size;
    size = backtrace(array, MAX_SIZE);
    getting_trace = 0;
    void *res = 0;
    for (i = 0; i < size; i++) {
	    res = array[i];
	    if ((size_t)array[i] < (size_t)0x700000000000) return res;
    }
    return res;
}

void print_trace3(void) {
    if (printing_trace) return;
    printing_trace = 1;
    size_t i, size;
    enum Constexpr { MAX_SIZE = 1024 };
    void *array[MAX_SIZE];
    fprintf(stderr, "S0\n");
    size = backtrace(array, MAX_SIZE);
    for (i = 0; i < size; i++)
        fprintf(stderr, "O: %p\n", array[i]);
    printing_trace = 0;
}

void *malloc(size_t size)
{

#ifdef COUNT_BYTES
    if (size == (~(size_t)0)) {
        // hack used to report memory usage bytes from all threads
        return (void *)mem_allocated_total_bytes;
    }
    if (size == (~(size_t)0) - 1) {
        // hack used to report memory usage bytes from current thread
        return (void *)mem_allocated_bytes;
    }

    void *ptr = __libc_malloc(size + ALIGN);
    if (ptr)  {
        if (tid == 0) tid = gettid();
        struct Header header = { MAGIC, size, tid, (size_t)get_trace(size)};
        *(struct Header*)ptr = header;
	mem_allocated_cnt += 1;
	mem_allocated_bytes += size;
	__atomic_add_fetch(&mem_allocated_total_bytes, size, __ATOMIC_SEQ_CST);

        return ptr + ALIGN;
    }
    return ptr;
#else
    return __libc_malloc(size);
#endif
}

void free(void *ptr)
{
#ifdef COUNT_BYTES
	if (ptr) ptr -= ALIGN;
#endif
	if (ptr && (long long)ptr % 16 != 0) {
        	fprintf(stderr, "BUG %p\n", ptr);
	}
	if (ptr && ((struct Header*)ptr)->magic != MAGIC) {
        	fprintf(stderr, "BUG_FREE %p\n", ptr);
		print_trace3();
		ptr += ALIGN;
	} else {
            if (ptr) {
    		((struct Header*)ptr)->magic += 0x100;
            	mem_allocated_cnt -= 1;
            	mem_allocated_bytes -= ((struct Header*)ptr)->size;
		__atomic_sub_fetch(&mem_allocated_total_bytes, ((struct Header*)ptr)->size, __ATOMIC_SEQ_CST);
	    }
	}
        __libc_free(ptr);
}

void *realloc(void *ptr, size_t size)
{
#ifdef COUNT_BYTES
    if (ptr) {ptr -= ALIGN; }
#endif


#ifdef COUNT_BYTES
    if (ptr && ((struct Header*)ptr)->magic != MAGIC) {
	ptr += ALIGN;
	fprintf(stderr, "BUG_REALLOC %p\n", ptr);
        print_trace3();
        return __libc_realloc(ptr, size);
    }
    if (ptr) {
        mem_allocated_bytes -= ((struct Header*)ptr)->size;
	__atomic_sub_fetch(&mem_allocated_total_bytes, ((struct Header*)ptr)->size, __ATOMIC_SEQ_CST);
    }

    if (tid == 0) tid = gettid();
    struct Header header;

    if (ptr) {
	    header = *((struct Header*)ptr);
	    ((struct Header*)ptr)->magic += 0x100;

    } else {
    	struct Header header2 = { MAGIC, size, tid, (size_t)get_trace(size)};
	header = header2;
    }

    void *nptr = __libc_realloc(ptr, size + ALIGN);
    if (nptr) {
    	   *(struct Header*)nptr = header;
           mem_allocated_bytes += size;
           __atomic_add_fetch(&mem_allocated_total_bytes, size, __ATOMIC_SEQ_CST);

	   return nptr + ALIGN;
    }
    return nptr;
#else
    return __libc_realloc(ptr, size);
#endif
}

void *calloc(size_t nmemb, size_t size)
{

#ifdef COUNT_BYTES
    void *ptr = __libc_calloc(1, nmemb*size + ALIGN);
    if (tid == 0) tid = gettid();
    struct Header header = { MAGIC, size, tid, (size_t)get_trace(size)};
    *(struct Header*)ptr = header;
    return ptr + ALIGN;
#else
    return __libc_calloc(nmemb, size);
#endif
}

void *memalign(size_t blocksize, size_t bytes)
{
#ifdef COUNT_BYTES
    return malloc(blocksize * bytes);
#else
    void *ptr = __libc_memalign(blocksize, bytes);
    return ptr;
#endif
}

int posix_memalign(void **memptr, size_t alignment, size_t size)
{
    if (myfn_posix_memalign == NULL) myfn_posix_memalign = dlsym(RTLD_NEXT, "posix_memalign");

    int res = myfn_posix_memalign(memptr, alignment, size + ALIGN);
    if (tid == 0) tid = gettid();
    struct Header header = { MAGIC, size, tid, (size_t)get_trace(size)};
    *(struct Header*)*memptr = header;
    mem_allocated_cnt += 1;
    mem_allocated_bytes += size;

    *memptr += ALIGN;

    return res;
}

void *valloc(size_t size) {
	fprintf(stderr, "NOT IMPLEMENTED VALLOC");
	return NULL;
}

void *alligned_alloc(size_t alignment,size_t size) {
	fprintf(stderr, "NOT IMPLEMENTED alligned_alloc");
	return NULL;
}

void *pvalloc(size_t lignment,size_t size) {
	fprintf(stderr, "NOT IMPLEMENTED pvalloc");
	return NULL;
}

void *mmap(void *addr, size_t length, int prot, int flags,
	  int fd, off_t offset) {
    if (myfn_mmap == NULL) myfn_mmap = dlsym(RTLD_NEXT, "mmap");

    	void *ptr =  myfn_mmap(addr, length, prot, flags, fd, offset);
	thread_init();
	fprintf(file, "mmap %p %ld %p %x\n", ptr, length, (void *)get_trace(length), flags);
    	fflush(file);

	return ptr;
}


int munmap(void *addr, size_t length) {
    if (myfn_munmap == NULL) myfn_munmap = dlsym(RTLD_NEXT, "munmap");

    int res = myfn_munmap(addr, length);
    thread_init();
    fprintf(file, "munmap %p %ld\n", addr, length);
    fflush(file);
    return res;
}

