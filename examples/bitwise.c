int popcount(long x) {
    int count = 0;
    while (x) {
        count += x & 1;
        x = x >> 1;
        if (x < 0) x = x & 0x7FFFFFFFFFFFFFFF;
    }
    return count;
}

int main() {
    long a = 0xF0F0;
    long b = 0x0FF0;
    printf("a & b = %x\n", a & b);
    printf("a | b = %x\n", a | b);
    printf("a ^ b = %x\n", a ^ b);
    printf("~a = %d\n", ~a);
    printf("1 << 10 = %d\n", 1 << 10);
    printf("-64 >> 3 = %d\n", -64 >> 3);
    printf("popcount(255) = %d\n", popcount(255));
    printf("popcount(-1) = %d\n", popcount(-1));
    return 0;
}
