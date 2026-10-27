int main() {
    int n = read_int();
    long total = 0;
    long largest = 0;
    for (int i = 0; i < n; i++) {
        long value = read_int();
        total += value;
        if (i == 0 || value > largest) largest = value;
    }
    printf("read %d numbers\n", n);
    printf("sum = %d, max = %d\n", total, largest);
    return 0;
}
