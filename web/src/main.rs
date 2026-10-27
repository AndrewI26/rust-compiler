use std::cell::Cell;
use std::rc::Rc;

use assembler::Program;
use compiler::{CompileError, Compilation, Outcome};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Element, HtmlDocument, HtmlSelectElement, HtmlTextAreaElement, KeyboardEvent};

const INDENT: &str = "    ";
const DELAY_MS: i32 = 300;

const EXAMPLES: [(&str, &str, &str); 8] = [
    ("Hello, world", include_str!("../../examples/hello.c"), ""),
    ("Fibonacci", include_str!("../../examples/fibonacci.c"), ""),
    ("Prime sieve", include_str!("../../examples/primes.c"), ""),
    ("Bubble sort", include_str!("../../examples/sort.c"), ""),
    ("Strings", include_str!("../../examples/strings.c"), ""),
    ("Bitwise operators", include_str!("../../examples/bitwise.c"), ""),
    ("Heap and 2D arrays", include_str!("../../examples/memory.c"), ""),
    ("Reading input", include_str!("../../examples/input.c"), "5\n3 9 -2 14 7\n"),
];

#[derive(Clone)]
struct Page {
    editor: HtmlTextAreaElement,
    input: HtmlTextAreaElement,
    backdrop: Element,
    gutter: Element,
    status: Element,
    output: Element,
    stages: [Element; 4],
}

fn main() {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let element = |id: &str| document.get_element_by_id(id).unwrap();
    let html_document: HtmlDocument = document.clone().dyn_into().unwrap();

    let page = Page {
        editor: element("editor").dyn_into().unwrap(),
        input: element("stdin").dyn_into().unwrap(),
        backdrop: element("backdrop"),
        gutter: element("gutter"),
        status: element("status"),
        output: element("output"),
        stages: [
            element("tokens"),
            element("ast"),
            element("assembly"),
            element("encode"),
        ],
    };
    let examples: HtmlSelectElement = element("examples").dyn_into().unwrap();

    examples.set_inner_html(
        &EXAMPLES
            .iter()
            .enumerate()
            .map(|(i, (name, _, _))| format!("<option value=\"{}\">{}</option>", i, escape(name)))
            .collect::<String>(),
    );
    page.editor.set_value(EXAMPLES[0].1);
    page.input.set_value(EXAMPLES[0].2);

    check(&page);
    render(&page);

    let rerender = {
        let page = page.clone();
        Closure::<dyn FnMut()>::new(move || render(&page))
    };
    let timer: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));
    let schedule = {
        let page = page.clone();
        move || {
            check(&page);
            page.status.set_class_name("status");
            page.status
                .set_inner_html("<span class=\"spinner\"></span>Compiling…");
            for stage in page.stages.iter().chain([&page.output]) {
                stage.set_class_name("stage-body loading");
            }
            if let Some(handle) = timer.take() {
                window.clear_timeout_with_handle(handle);
            }
            let handle = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    rerender.as_ref().unchecked_ref(),
                    DELAY_MS,
                )
                .unwrap();
            timer.set(Some(handle));
        }
    };
    let schedule = Rc::new(std::cell::RefCell::new(schedule));

    listen(&page.editor, "input", {
        let schedule = schedule.clone();
        move || (schedule.borrow_mut())()
    });
    listen(&page.input, "input", {
        let schedule = schedule.clone();
        move || (schedule.borrow_mut())()
    });
    let select: Element = examples.clone().into();
    listen(&select, "change", {
        let page = page.clone();
        let schedule = schedule.clone();
        move || {
            let index: usize = examples.value().parse().unwrap_or(0);
            let (_, source, input) = EXAMPLES[index];
            page.editor.set_value(source);
            page.input.set_value(input);
            page.editor.set_scroll_top(0);
            (schedule.borrow_mut())();
        }
    });

    let on_keydown = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
        if event.key() == "Tab" && !event.shift_key() {
            event.prevent_default();
            let _ = html_document.exec_command_with_show_ui_and_value("insertText", false, INDENT);
        }
    });
    page.editor
        .add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref())
        .unwrap();
    on_keydown.forget();

    listen(&page.editor, "scroll", {
        let page = page.clone();
        move || sync_scroll(&page)
    });
}

fn check(page: &Page) {
    let source = page.editor.value();
    let errors = compiler::analyze(&source).errors;
    page.backdrop
        .set_inner_html(&render_backdrop(&source, &errors));
    page.gutter.set_inner_html(&render_gutter(&source, &errors));
    sync_scroll(page);
}

fn render(page: &Page) {
    let compilation = compiler::analyze(&page.editor.value());
    let program = compilation.assembly.as_deref().map(assembler::analyze);

    page.stages[0].set_inner_html(&render_tokens(&compilation));
    page.stages[1].set_inner_html(&render_ast(&compilation));
    page.stages[2].set_inner_html(&render_assembly(&compilation));
    page.stages[3].set_inner_html(&render_encode(program.as_ref()));

    let words = program
        .as_ref()
        .filter(|p| p.errors.is_empty())
        .map(|p| {
            p.items
                .iter()
                .flat_map(|item| item.item.encode())
                .collect::<Vec<u32>>()
        });
    let outcome = words
        .as_ref()
        .map(|words| compiler::execute(words, &page.input.value()));
    page.output.set_inner_html(&render_output(outcome.as_ref()));

    for stage in page.stages.iter().chain([&page.output]) {
        stage.set_class_name("stage-body");
    }

    let (class, text) = describe_status(&compilation, program.as_ref(), words.as_deref());
    page.status.set_class_name(class);
    page.status.set_text_content(Some(&text));
}

fn describe_status(
    compilation: &Compilation,
    program: Option<&Program>,
    words: Option<&[u32]>,
) -> (&'static str, String) {
    let errors = compilation.errors.len();
    if errors > 0 {
        return ("status error", plural(errors, "error"));
    }
    if let Some(program) = program.filter(|p| !p.errors.is_empty()) {
        return (
            "status error",
            plural(program.errors.len(), "assembler error"),
        );
    }
    let words = words.map_or(0, |w| w.len());
    ("status", format!("✓ Compiled · {}", plural(words, "word")))
}

fn plural(count: usize, noun: &str) -> String {
    format!("{} {}{}", count, noun, if count == 1 { "" } else { "s" })
}

fn listen(target: &Element, event: &str, callback: impl FnMut() + 'static) {
    let closure = Closure::<dyn FnMut()>::new(callback);
    target
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .unwrap();
    closure.forget();
}

fn sync_scroll(page: &Page) {
    page.backdrop.set_scroll_top(page.editor.scroll_top());
    page.backdrop.set_scroll_left(page.editor.scroll_left());
    page.gutter.set_scroll_top(page.editor.scroll_top());
}

fn render_backdrop(source: &str, errors: &[CompileError]) -> String {
    source
        .split('\n')
        .enumerate()
        .map(|(i, line)| {
            let messages = messages_for(i + 1, errors);
            if messages.is_empty() {
                return format!("<div class=\"line\">{}</div>", escape(line));
            }
            let code = line.trim();
            let start = line.find(code).unwrap_or(0);
            let (before, rest) = line.split_at(start);
            let after = &rest[code.len()..];
            format!(
                "<div class=\"line error\">{}<span class=\"squiggle\">{}</span>{}<span class=\"message\">{}</span></div>",
                escape(before),
                escape(code),
                escape(after),
                escape(&messages.join(" · ")),
            )
        })
        .collect()
}

fn render_gutter(source: &str, errors: &[CompileError]) -> String {
    (1..=source.split('\n').count())
        .map(|line| {
            let class = if messages_for(line, errors).is_empty() {
                ""
            } else {
                " error"
            };
            format!("<div class=\"number{}\">{}</div>", class, line)
        })
        .collect()
}

fn messages_for(line: usize, errors: &[CompileError]) -> Vec<&str> {
    errors
        .iter()
        .filter(|e| e.line == line)
        .map(|e| e.message.as_str())
        .collect()
}

fn render_errors(compilation: &Compilation) -> String {
    let rows: String = compilation
        .errors
        .iter()
        .map(|error| {
            format!(
                "<tr><td class=\"ln\">{}</td><td class=\"error\">{}</td></tr>",
                error.line,
                escape(&error.message)
            )
        })
        .collect();
    table_or_empty(&rows, "No errors")
}

fn render_tokens(compilation: &Compilation) -> String {
    if compilation.tokens.is_empty() {
        return render_errors(compilation);
    }
    let mut rows = String::new();
    let mut current: Option<usize> = None;
    for token in &compilation.tokens {
        let (kind, lexeme) = token.describe();
        if kind == "EOF" {
            continue;
        }
        if current != Some(token.line) {
            if current.is_some() {
                rows.push_str("</td></tr>");
            }
            rows.push_str(&format!("<tr><td class=\"ln\">{}</td><td>", token.line));
            current = Some(token.line);
        }
        rows.push_str(&format!(
            "<span class=\"token\"><span class=\"kind\">{}</span>{}</span>",
            kind,
            escape(&lexeme)
        ));
    }
    if current.is_some() {
        rows.push_str("</td></tr>");
    }
    table_or_empty(&rows, "No tokens")
}

fn render_ast(compilation: &Compilation) -> String {
    match &compilation.program {
        Some(program) => {
            let tree = compiler::ast::render(program);
            if tree.is_empty() {
                return String::from("<p class=\"empty\">Empty program</p>");
            }
            let lines: String = tree
                .lines()
                .map(|line| {
                    let indent = line.len() - line.trim_start().len();
                    let text = line.trim_start();
                    let (head, tail) = text.split_once(' ').unwrap_or((text, ""));
                    format!(
                        "<div class=\"node\" style=\"padding-left:{}ch\"><span class=\"node-kind\">{}</span> {}</div>",
                        indent,
                        escape(head),
                        escape(tail)
                    )
                })
                .collect();
            format!("<div class=\"tree\">{}</div>", lines)
        }
        None => render_errors(compilation),
    }
}

fn render_assembly(compilation: &Compilation) -> String {
    match &compilation.assembly {
        Some(assembly) => {
            let rows: String = assembly
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    let class = if line.ends_with(':') || line.contains(": .8byte") {
                        " class=\"label\""
                    } else {
                        ""
                    };
                    format!(
                        "<tr><td class=\"ln\">{}</td><td{}>{}</td></tr>",
                        i + 1,
                        class,
                        escape(line)
                    )
                })
                .collect();
            format!("<table class=\"code-table\">{}</table>", rows)
        }
        None => render_errors(compilation),
    }
}

fn render_encode(program: Option<&Program>) -> String {
    let Some(program) = program else {
        return String::from("<p class=\"empty\">Fix the errors to see the machine code</p>");
    };
    if !program.errors.is_empty() {
        let rows: String = program
            .errors
            .iter()
            .map(|e| {
                format!(
                    "<tr><td class=\"ln\">{}</td><td class=\"error\">{}</td></tr>",
                    e.line,
                    escape(&e.message)
                )
            })
            .collect();
        return table_or_empty(&rows, "No errors");
    }
    let rows: String = program
        .items
        .iter()
        .flat_map(|parsed| {
            let fields = parsed.item.fields();
            parsed
                .item
                .encode()
                .into_iter()
                .enumerate()
                .map(move |(i, word)| {
                    format!(
                        "<tr><td class=\"ln\">0x{:04X}</td><td>{:08X}</td><td>{}</td></tr>",
                        (parsed.index + i as i32) * 4,
                        word,
                        group_bits(word, fields)
                    )
                })
        })
        .collect();
    table_or_empty(&rows, "No machine code")
}

fn render_output(outcome: Option<&Outcome>) -> String {
    let Some(outcome) = outcome else {
        return String::from("<p class=\"empty\">Fix the errors to run the program</p>");
    };
    let mut html = String::new();
    if outcome.output.is_empty() {
        html.push_str("<p class=\"empty\">No output</p>");
    } else {
        html.push_str(&format!("<pre class=\"stdout\">{}</pre>", escape(&outcome.output)));
    }
    if let Some(error) = &outcome.error {
        html.push_str(&format!("<p class=\"error\">{}</p>", escape(error)));
    }
    let exit = outcome
        .exit_code
        .map_or(String::from("—"), |code| code.to_string());
    html.push_str(&format!(
        "<p class=\"meta\">exit code <b>{}</b> · {} executed</p>",
        exit,
        plural(outcome.steps as usize, "instruction")
    ));
    html
}

fn group_bits(word: u32, widths: &[u32]) -> String {
    let bits = format!("{:032b}", word);
    let mut start = 0;
    widths
        .iter()
        .map(|width| {
            let end = start + *width as usize;
            let field = format!("<span class=\"field\">{}</span>", &bits[start..end]);
            start = end;
            field
        })
        .collect()
}

fn table_or_empty(rows: &str, empty: &str) -> String {
    if rows.is_empty() {
        format!("<p class=\"empty\">{}</p>", empty)
    } else {
        format!("<table>{}</table>", rows)
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squiggles_only_the_code_on_error_lines() {
        let errors = vec![CompileError {
            line: 2,
            message: String::from("bad <thing>"),
        }];
        assert_eq!(
            render_backdrop("ok\n  foo x1 ", &errors),
            "<div class=\"line\">ok</div>\
             <div class=\"line error\">  <span class=\"squiggle\">foo x1</span> \
             <span class=\"message\">bad &lt;thing&gt;</span></div>"
        );
    }

    #[test]
    fn marks_error_lines_in_gutter() {
        let errors = vec![CompileError {
            line: 2,
            message: String::new(),
        }];
        assert_eq!(
            render_gutter("a\nb", &errors),
            "<div class=\"number\">1</div><div class=\"number error\">2</div>"
        );
    }

    #[test]
    fn groups_bits_by_field() {
        assert_eq!(
            group_bits(0x14000001, &[6, 26]),
            "<span class=\"field\">000101</span>\
             <span class=\"field\">00000000000000000000000001</span>"
        );
    }

    #[test]
    fn every_example_compiles_and_runs() {
        for (name, source, input) in EXAMPLES {
            let outcome = compiler::run(source, input).unwrap_or_else(|_| panic!("{}", name));
            assert!(outcome.error.is_none(), "{}", name);
        }
    }

    #[test]
    fn status_counts_errors() {
        let compilation = compiler::analyze("int main() { return x; }");
        assert_eq!(
            describe_status(&compilation, None, None),
            ("status error", String::from("1 error"))
        );
    }

    #[test]
    fn shows_program_output() {
        let outcome = compiler::run("int main() { printf(\"<hi>\"); return 3; }", "").unwrap();
        let html = render_output(Some(&outcome));
        assert!(html.contains("&lt;hi&gt;"));
        assert!(html.contains("exit code <b>3</b>"));
    }
}
