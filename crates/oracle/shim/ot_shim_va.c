/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Copyright (C) rustyrian contributors */

/*
 * Variadic SDL logging shims. Rust cannot define C-variadic functions, so these
 * format with vsnprintf into 1,024 bytes and hand the message to Rust.
 */

#include "ot_shim.h"

#include <stdarg.h>
#include <stdio.h>

void ot_shim_rs_log(const char *name, int category, const char *message);

#define OT_FORMAT(fmt) \
	char buffer[1024]; \
	va_list ap; \
	va_start(ap, fmt); \
	vsnprintf(buffer, sizeof buffer, fmt, ap); \
	va_end(ap)

void ot_shim_SDL_Log(const char *fmt, ...)
{
	OT_FORMAT(fmt);
	ot_shim_rs_log("SDL_Log", SDL_LOG_CATEGORY_APPLICATION, buffer);
}

void ot_shim_SDL_LogDebug(int category, const char *fmt, ...)
{
	OT_FORMAT(fmt);
	ot_shim_rs_log("SDL_LogDebug", category, buffer);
}

void ot_shim_SDL_LogWarn(int category, const char *fmt, ...)
{
	OT_FORMAT(fmt);
	ot_shim_rs_log("SDL_LogWarn", category, buffer);
}

void ot_shim_SDL_LogError(int category, const char *fmt, ...)
{
	OT_FORMAT(fmt);
	ot_shim_rs_log("SDL_LogError", category, buffer);
}
