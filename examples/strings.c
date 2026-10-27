void reverse(char *s) {
    char *end = s + strlen(s) - 1;
    while (s < end) {
        char t = *s;
        *s++ = *end;
        *end-- = t;
    }
}

int is_palindrome(char *s) {
    int n = strlen(s);
    for (int i = 0; i < n / 2; i++) {
        if (s[i] != s[n - 1 - i]) return 0;
    }
    return 1;
}

int main() {
    char word[32];
    strcpy(word, "compiler");
    printf("%s has %d letters\n", word, strlen(word));
    reverse(word);
    printf("reversed: %s\n", word);

    char *candidates[3] = {"racecar", "level", "rust"};
    for (int i = 0; i < 3; i++) {
        printf("%s: %s\n", candidates[i], is_palindrome(candidates[i]) ? "palindrome" : "not a palindrome");
    }
    return 0;
}
