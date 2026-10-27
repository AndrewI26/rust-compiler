void swap(int *a, int *b) {
    int t = *a;
    *a = *b;
    *b = t;
}

void bubble_sort(int *items, int n) {
    for (int i = 0; i < n - 1; i++) {
        for (int j = 0; j < n - 1 - i; j++) {
            if (items[j] > items[j + 1]) swap(&items[j], &items[j + 1]);
        }
    }
}

void print_array(int *items, int n) {
    for (int i = 0; i < n; i++) printf("%d ", items[i]);
    putchar('\n');
}

int main() {
    int numbers[] = {42, -7, 19, 3, 88, 0, -21, 5};
    int n = sizeof(numbers) / sizeof(numbers[0]);
    print_array(numbers, n);
    bubble_sort(numbers, n);
    print_array(numbers, n);
    return 0;
}
