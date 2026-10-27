#define LIMIT 100

int sieve[LIMIT + 1];

int main() {
    int count = 0;
    for (int i = 2; i <= LIMIT; i++) {
        if (sieve[i]) continue;
        printf("%d ", i);
        count++;
        for (int j = i * i; j <= LIMIT; j += i) sieve[j] = 1;
    }
    printf("\n%d primes below %d\n", count, LIMIT);
    return count;
}
