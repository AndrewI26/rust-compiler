int *make_squares(int n) {
    int *squares = malloc(n * sizeof(int));
    for (int i = 0; i < n; i++) squares[i] = i * i;
    return squares;
}

long sum(int *items, int n) {
    long total = 0;
    for (int *p = items; p < items + n; p++) total += *p;
    return total;
}

int grid[3][4];

int main() {
    int *squares = make_squares(10);
    printf("sum of the first 10 squares: %d\n", sum(squares, 10));

    for (int r = 0; r < 3; r++)
        for (int c = 0; c < 4; c++)
            grid[r][c] = r * 10 + c;
    for (int r = 0; r < 3; r++) {
        for (int c = 0; c < 4; c++) printf("%d\t", grid[r][c]);
        putchar('\n');
    }
    return 0;
}
