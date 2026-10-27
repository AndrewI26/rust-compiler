long __heap;

void *malloc(long size) {
    if (__heap == 0) __heap = (long)__heap_start();
    long p = __heap;
    __heap = __heap + (size + 7) / 8 * 8;
    return (void *)p;
}

void *calloc(long count, long size) {
    long *p = malloc(count * size);
    long words = (count * size + 7) / 8;
    for (long i = 0; i < words; i++) p[i] = 0;
    return p;
}

void free(void *p) {}

long abs(long n) {
    return n < 0 ? -n : n;
}

void print_str(char *s) {
    while (*s) putchar(*s++);
}

int puts(char *s) {
    print_str(s);
    putchar('\n');
    return 0;
}

void print_int(long n) {
    if (n < 0) {
        putchar('-');
        if (n < -9) print_int(-(n / 10));
        putchar('0' - n % 10);
        return;
    }
    if (n > 9) print_int(n / 10);
    putchar('0' + n % 10);
}

long read_int(void) {
    long c = getchar();
    while (c == ' ' || c == '\n' || c == '\t' || c == '\r') c = getchar();
    long sign = 1;
    if (c == '-') {
        sign = -1;
        c = getchar();
    }
    long n = 0;
    while (c >= '0' && c <= '9') {
        n = n * 10 + c - '0';
        c = getchar();
    }
    return n * sign;
}

long strlen(char *s) {
    long n = 0;
    while (s[n]) n++;
    return n;
}

int strcmp(char *a, char *b) {
    while (*a && *a == *b) {
        a++;
        b++;
    }
    return *a - *b;
}

char *strcpy(char *dest, char *src) {
    char *start = dest;
    while ((*dest++ = *src++));
    return start;
}

long __halve(long a) {
    if (a % 2 != 0) return (a - 1) / 2;
    return a / 2;
}

long __shl(long a, long n) {
    while (n > 0 && a != 0) {
        a = a * 2;
        n--;
    }
    return a;
}

long __shr(long a, long n) {
    while (n > 0 && a != 0 && a != -1) {
        a = __halve(a);
        n--;
    }
    return a;
}

long __bit(long x, long y, long op) {
    if (op == 0) return x && y;
    if (op == 1) return x || y;
    return x != y;
}

long __bitwise(long a, long b, long op) {
    long result = 0;
    long bit = 1;
    for (;;) {
        if ((a == 0 || a == -1) && (b == 0 || b == -1)) {
            if (__bit(a != 0, b != 0, op)) result = result - bit;
            return result;
        }
        if (__bit(a % 2 != 0, b % 2 != 0, op)) result = result + bit;
        bit = bit * 2;
        a = __halve(a);
        b = __halve(b);
    }
}

long __and(long a, long b) {
    return __bitwise(a, b, 0);
}

long __or(long a, long b) {
    return __bitwise(a, b, 1);
}

long __xor(long a, long b) {
    return __bitwise(a, b, 2);
}

void __print_hex(long n) {
    char digits[16];
    long count = 0;
    do {
        long d = n % 16;
        if (d < 0) d = d + 16;
        digits[count++] = d < 10 ? '0' + d : 'a' + d - 10;
        n = __shr(n, 4);
    } while (n != 0 && count < 16);
    while (count > 0) putchar(digits[--count]);
}
