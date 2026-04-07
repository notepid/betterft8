/* MSVC compatibility shim for POSIX functions used by ft8_lib */
#pragma once

#ifdef _MSC_VER
#include <string.h>

/* stpcpy: copies src to dest and returns pointer to the NUL terminator in dest */
static __inline char* stpcpy(char* dest, const char* src)
{
    while ((*dest++ = *src++) != '\0');
    return dest - 1;
}
#endif /* _MSC_VER */
