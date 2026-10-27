fn run(source: &str) -> (String, i64) {
    run_with_input(source, "")
}

fn run_with_input(source: &str, input: &str) -> (String, i64) {
    let outcome = match compiler::run(source, input) {
        Ok(outcome) => outcome,
        Err(errors) => panic!("compile errors: {:?}", errors),
    };
    if let Some(error) = outcome.error {
        panic!("runtime error: {}", error);
    }
    (outcome.output, outcome.exit_code.unwrap())
}

fn exit_code(source: &str) -> i64 {
    run(source).1
}

fn expr(expression: &str) -> i64 {
    exit_code(&format!("long main() {{ return {}; }}", expression))
}

fn error(source: &str) -> (usize, String) {
    let errors = compiler::compile(source).unwrap_err();
    (errors[0].line, errors[0].message.clone())
}

fn example(name: &str) -> String {
    let path = format!("{}/../examples/{}.c", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn arithmetic() {
    assert_eq!(expr("1 + 2 * 3"), 7);
    assert_eq!(expr("(1 + 2) * 3"), 9);
    assert_eq!(expr("17 / 5"), 3);
    assert_eq!(expr("-17 / 5"), -3);
    assert_eq!(expr("17 % 5"), 2);
    assert_eq!(expr("-17 % 5"), -2);
    assert_eq!(expr("-(3 - 10)"), 7);
    assert_eq!(expr("1000000 * 1000000"), 1_000_000_000_000);
    assert_eq!(expr("9223372036854775807 + 1"), i64::MIN);
}

#[test]
fn comparisons_and_logic() {
    assert_eq!(expr("3 < 5"), 1);
    assert_eq!(expr("5 <= 4"), 0);
    assert_eq!(expr("-1 > -2"), 1);
    assert_eq!(expr("2 == 2 && 3 != 3"), 0);
    assert_eq!(expr("0 || 7"), 1);
    assert_eq!(expr("!0 + !5"), 1);
    assert_eq!(expr("1 ? 10 : 20"), 10);
    assert_eq!(expr("0 ? 10 : 20"), 20);
}

#[test]
fn bitwise() {
    assert_eq!(expr("12 & 10"), 8);
    assert_eq!(expr("12 | 10"), 14);
    assert_eq!(expr("12 ^ 10"), 6);
    assert_eq!(expr("~0"), -1);
    assert_eq!(expr("-8 & 0xFF"), 248);
    assert_eq!(expr("-1 ^ 5"), -6);
    assert_eq!(expr("1 << 62"), 1 << 62);
    assert_eq!(expr("-100 >> 2"), -25);
    assert_eq!(expr("-7 >> 1"), -4);
    assert_eq!(expr("0x7F00 >> 8"), 0x7F);
    assert_eq!(expr("(long)1 << 63"), i64::MIN);
}

#[test]
fn short_circuit_skips_side_effects() {
    let source = "
int calls;
int touch() { calls++; return 1; }
int main() {
    0 && touch();
    1 || touch();
    1 && touch();
    0 || touch();
    return calls;
}";
    assert_eq!(exit_code(source), 2);
}

#[test]
fn compound_assignment_and_increments() {
    let source = "
int main() {
    int x = 5;
    x += 3;
    x *= 2;
    x -= 1;
    x /= 3;
    x %= 4;
    x <<= 4;
    x |= 3;
    x ^= 1;
    x >>= 1;
    int a = x++;
    int b = ++x;
    int c = x--;
    return a * 10000 + b * 100 + c - --x;
}";
    assert_eq!(exit_code(source), 9 * 10000 + 11 * 100 + 11 - 9);
}

#[test]
fn loops_break_and_continue() {
    let source = "
int main() {
    int total = 0;
    for (int i = 0; i < 100; i++) {
        if (i % 2) continue;
        if (i > 10) break;
        total += i;
    }
    int j = 0;
    while (1) { j++; if (j == 7) break; }
    int k = 0;
    do { k += 3; } while (k < 10);
    return total * 100 + j * 10 + k - 12;
}";
    assert_eq!(exit_code(source), 30 * 100 + 70);
}

#[test]
fn nested_scopes_shadow() {
    let source = "
int x = 1;
int main() {
    int total = x;
    int x = 10;
    { int x = 100; total += x; }
    total += x;
    for (int x = 0; x < 1; x++) total += 1000;
    return total + x;
}";
    assert_eq!(exit_code(source), 1 + 100 + 10 + 1000 + 10);
}

#[test]
fn recursion_and_many_arguments() {
    let source = "
long ackermann(long m, long n) {
    if (m == 0) return n + 1;
    if (n == 0) return ackermann(m - 1, 1);
    return ackermann(m - 1, ackermann(m, n - 1));
}
long weigh(long a, long b, long c, long d, long e, long f, long g, long h, long i) {
    return a + 2*b + 3*c + 4*d + 5*e + 6*f + 7*g + 8*h + 9*i;
}
int main() { return ackermann(2, 3) * 1000 + weigh(1, 1, 1, 1, 1, 1, 1, 1, 1); }";
    assert_eq!(exit_code(source), 9 * 1000 + 45);
}

#[test]
fn calls_inside_deep_expressions_preserve_temporaries() {
    let source = "
int id(int x) { return x; }
int main() { return 1 + (2 * (3 + id(4) * (5 + id(id(6) + 1)))) - id(7) % 4; }";
    assert_eq!(exit_code(source), 1 + 2 * (3 + 4 * (5 + 7)) - 3);
}

#[test]
fn pointers_and_arrays() {
    let source = "
int main() {
    int a[5] = {1, 2, 3};
    int *p = a;
    *(p + 3) = 40;
    p[4] = 50;
    int **pp = &p;
    (*pp)++;
    long distance = &a[4] - p;
    return a[0] + a[1] + a[2] + a[3] + a[4] + *p * 100 + distance * 10000;
}";
    assert_eq!(exit_code(source), 96 + 200 + 30000);
}

#[test]
fn multidimensional_arrays() {
    let source = "
int m[3][3] = {{1, 2, 3}, {4, 5, 6}, {7, 8, 9}};
int main() {
    int trace = 0;
    for (int i = 0; i < 3; i++) trace += m[i][i];
    int local[2][2] = {{1}, {0, 1}};
    return trace * 100 + sizeof(m) / sizeof(m[0]) * 10 + local[0][0] + local[1][1] + local[0][1];
}";
    assert_eq!(exit_code(source), 1500 + 30 + 2);
}

#[test]
fn large_frames() {
    let source = "
int main() {
    int big[100];
    for (int i = 0; i < 100; i++) big[i] = i;
    int after = 7;
    long sum = 0;
    for (int i = 0; i < 100; i++) sum += big[i];
    return sum + after;
}";
    assert_eq!(exit_code(source), 4950 + 7);
}

#[test]
fn globals_and_strings() {
    let source = r#"
char *greeting = "hi";
char name[] = "bob";
int counter = 3;
int main() {
    printf("%s %s %d%c", greeting, name, counter, '!');
    return sizeof(name);
}"#;
    assert_eq!(run(source), (String::from("hi bob 3!"), 32));
}

#[test]
fn printf_formats() {
    let source = r#"int main() { printf("%d|%i|%c|%s|%x|%ld|100%%\n", -42, 7, 'z', "str", 255, 1234567890123); return 0; }"#;
    assert_eq!(run(source).0, "-42|7|z|str|ff|1234567890123|100%\n");
}

#[test]
fn print_extremes() {
    let source = "int main() { print_int(-9223372036854775807 - 1); putchar(' '); __print_hex(-1); return 0; }";
    assert_eq!(run(source).0, "-9223372036854775808 ffffffffffffffff");
}

#[test]
fn input() {
    let source = "
int main() {
    int c;
    int count = 0;
    while ((c = getchar()) != -1) if (c == 'a') count++;
    return count;
}";
    assert_eq!(run_with_input(source, "banana").1, 3);
    assert_eq!(
        run_with_input(
            "int main() { return read_int() + read_int(); }",
            "  12\n-5 "
        )
        .1,
        7
    );
}

#[test]
fn exit_stops_immediately() {
    assert_eq!(
        run("void stop() { exit(9); } int main() { putchar('a'); stop(); putchar('b'); return 0; }"),
        (String::from("a"), 9)
    );
}

#[test]
fn malloc_gives_distinct_memory() {
    let source = "
int main() {
    long *a = malloc(3 * sizeof(long));
    long *b = calloc(2, sizeof(long));
    a[0] = 1; a[1] = 2; a[2] = 3;
    return (b - a) * 10 + b[0] + b[1];
}";
    assert_eq!(exit_code(source), 30);
}

#[test]
fn macros_and_comments() {
    let source = "
#include <stdio.h>
#define SIZE 4
#define DOUBLE_SIZE (SIZE * 2)
/* block
   comment */
int main() {
    int a[DOUBLE_SIZE];
    return sizeof(a) / sizeof(int);
}";
    assert_eq!(exit_code(source), 8);
}

#[test]
fn user_functions_override_the_runtime() {
    let source = "long abs(long x) { return 99; } int main() { return abs(-5); }";
    assert_eq!(exit_code(source), 99);
}

#[test]
fn prototypes_allow_mutual_recursion() {
    let source = "
int is_odd(int n);
int is_even(int n) { return n == 0 ? 1 : is_odd(n - 1); }
int is_odd(int n) { return n == 0 ? 0 : is_even(n - 1); }
int main() { return is_even(10) * 10 + is_odd(7); }";
    assert_eq!(exit_code(source), 11);
}

#[test]
fn every_example_runs() {
    let cases = [
        ("hello", "Hello, world!\n2 + 3 = 5\n"),
        ("sort", "42 -7 19 3 88 0 -21 5 \n-21 -7 0 3 5 19 42 88 \n"),
        ("bitwise", "a & b = f0\na | b = fff0\na ^ b = ff00\n~a = -61681\n1 << 10 = 1024\n-64 >> 3 = -8\npopcount(255) = 8\npopcount(-1) = 64\n"),
    ];
    for (name, expected) in cases {
        assert_eq!(run(&example(name)).0, expected, "{}", name);
    }
    assert!(run(&example("fibonacci")).0.ends_with("fib(15) = 610\n"));
    assert_eq!(run(&example("primes")).1, 25);
    assert!(run(&example("strings")).0.contains("reversed: relipmoc"));
    assert!(run(&example("memory")).0.contains("285"));
    assert_eq!(
        run_with_input(&example("input"), "3 4 -1 9").0,
        "read 3 numbers\nsum = 12, max = 9\n"
    );
}

#[test]
fn errors() {
    assert_eq!(
        error("int main() {\n  return y;\n}"),
        (2, String::from("Unknown variable `y`"))
    );
    assert_eq!(
        error("int main() {\n  foo();\n}"),
        (2, String::from("Unknown function `foo`"))
    );
    assert_eq!(
        error("int f(int a) { return a; }\nint main() { return f(1, 2); }"),
        (2, String::from("`f` takes 1 argument but was given 2"))
    );
    assert_eq!(
        error("int main() { int a[2]; a = 0; }").1,
        "Cannot assign to an array"
    );
    assert_eq!(
        error("int main() {\n int x;\n int x;\n}"),
        (3, String::from("`x` is already declared in this scope"))
    );
    assert_eq!(error("int main() { break; }").1, "`break` or `continue` outside of a loop");
    assert_eq!(error("int f() { return 0; }").1, "The program has no `main` function");
    assert_eq!(error("int main() { int x; return *x; }").1, "Cannot dereference a value of type int");
    assert_eq!(error("int main() { 3 = 4; }").1, "This expression is not assignable");
    assert_eq!(error("int main() { int a[2] = {1, 2, 3}; }").1, "Too many values in the initializer");
    assert_eq!(error("int x = y;\nint main() {}").1, "The initializer of global `x` must be a constant");
    assert_eq!(error("int main() { printf(\"%d\"); }").1, "printf has fewer arguments than formats");
}

#[test]
fn errors_in_several_functions_are_all_reported() {
    let errors = compiler::compile("int f() { return a; }\nint main() { return b; }").unwrap_err();
    assert_eq!(errors.len(), 2);
}

#[test]
fn infinite_loops_are_stopped() {
    let outcome = compiler::run("int main() { while (1); }", "").unwrap();
    assert!(outcome.error.unwrap().contains("Stopped"));
}
