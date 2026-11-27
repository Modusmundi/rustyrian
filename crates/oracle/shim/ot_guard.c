/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Copyright (C) rustyrian contributors */

/*
 * `exit` and allocator shims, and `ot_guard_call`, which lets an in-process
 * differential test call a C function that may `exit`.
 */

#include "ot_shim.h"

#undef exit
#undef malloc
#undef calloc
#undef realloc
#undef free

#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* Implemented in Rust (`shim.rs`): the size of every live block allocated by
 * upstream, so mirrors can copy heap arrays whose length no global records.
 * `ot_shim_rs_forget` returns (size_t)-1 for a block it does not know. */
void ot_shim_rs_alloc(void *ptr, size_t size);
size_t ot_shim_rs_forget(void *ptr);
#define OT_UNKNOWN_SIZE ((size_t)-1)
/* Implemented in Rust: records the exit; in a runner, runs the registered
 * atexit handlers and finishes the trace. */
void ot_shim_rs_on_exit(int code, int guarded);

static jmp_buf *guard_buf = NULL;
static int guard_code = 0;

void ot_shim_exit(int code)
{
	ot_shim_rs_on_exit(code, guard_buf != NULL);
	if (guard_buf != NULL)
	{
		jmp_buf *buf = guard_buf;
		guard_buf = NULL;
		guard_code = code;
		longjmp(*buf, 1);
	}
	exit(code);
}

/* Allocations are zero-filled so upstream reads of unwritten memory are
 * deterministic (destruct.c:1460-1467 reads before writing). Each block is
 * also followed by OT_SLACK zero bytes, so upstream's small over-reads
 * (UF-4: song 8 reads one SoundBank past `soundbank`) read zero in every run,
 * the value the port's guards use. Sanitized builds get no slack, so ASan
 * still reports every overflow. */
#if defined(__has_feature)
#if __has_feature(address_sanitizer)
#define OT_SLACK 0
#endif
#endif
#ifndef OT_SLACK
#define OT_SLACK 256
#endif

void *ot_shim_malloc(size_t size)
{
	if (size > SIZE_MAX - OT_SLACK)
		return NULL;
	void *p = calloc(1, size + OT_SLACK);
	if (p != NULL)
		ot_shim_rs_alloc(p, size);
	return p;
}

void *ot_shim_calloc(size_t count, size_t size)
{
	if (size != 0 && count > (SIZE_MAX - OT_SLACK) / size)
		return NULL;
	void *p = calloc(1, count * size + OT_SLACK);
	if (p != NULL)
		ot_shim_rs_alloc(p, count * size);
	return p;
}

void *ot_shim_realloc(void *old, size_t size)
{
	if (size > SIZE_MAX - OT_SLACK)
		return NULL;
	size_t old_size = old != NULL ? ot_shim_rs_forget(old) : 0;
	void *p = realloc(old, size + OT_SLACK);
	if (p == NULL)
	{
		/* failed: the old block is still live (a zero-size realloc may
		 * instead have freed it) */
		if (old != NULL && size + OT_SLACK != 0 && old_size != OT_UNKNOWN_SIZE)
			ot_shim_rs_alloc(old, old_size);
		return NULL;
	}
	if (old_size != OT_UNKNOWN_SIZE && size > old_size)
		memset((char *)p + old_size, 0, size - old_size);
	memset((char *)p + size, 0, OT_SLACK);
	ot_shim_rs_alloc(p, size);
	return p;
}

void ot_shim_free(void *ptr)
{
	if (ptr != NULL)
		ot_shim_rs_forget(ptr);
	free(ptr);
}

/*
 * Calls fn(ctx). Returns 0 if it returned normally, or 1 if it called exit(),
 * storing the exit code in *code. The Rust callback must hold no values with
 * destructors.
 */
int ot_guard_call(void (*fn)(void *), void *ctx, int *code)
{
	jmp_buf buf;
	jmp_buf *prev = guard_buf;
	guard_buf = &buf;
	if (setjmp(buf) == 0)
	{
		fn(ctx);
		guard_buf = prev;
		return 0;
	}
	guard_buf = prev;
	*code = guard_code;
	return 1;
}
