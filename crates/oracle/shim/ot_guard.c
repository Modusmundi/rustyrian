/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Copyright (C) rustyrian contributors */

/*
 * `exit` and `malloc` shims, and `ot_guard_call`, which lets an in-process
 * differential test call a C function that may `exit`.
 */

#include "ot_shim.h"

#undef exit
#undef malloc

#include <setjmp.h>
#include <stdlib.h>

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

/* Upstream reads some allocations before writing them (destruct.c:1460-1467). */
void *ot_shim_malloc(size_t size)
{
	return calloc(1, size);
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
