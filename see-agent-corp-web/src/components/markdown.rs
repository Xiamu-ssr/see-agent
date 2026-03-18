use pulldown_cmark::{html, Parser};

pub fn render_markdown(md: &str) -> String {
    let parser = Parser::new(md);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_paragraph() {
        let result = render_markdown("hello world");
        assert!(result.contains("<p>hello world</p>"));
    }

    #[test]
    fn renders_bold() {
        let result = render_markdown("**bold**");
        assert!(result.contains("<strong>bold</strong>"));
    }

    #[test]
    fn renders_code_block() {
        let result = render_markdown("```\nlet x = 1;\n```");
        assert!(result.contains("<code>"));
        assert!(result.contains("let x = 1;"));
    }

    #[test]
    fn renders_empty_string() {
        let result = render_markdown("");
        assert!(result.is_empty());
    }

    #[test]
    fn renders_heading() {
        let result = render_markdown("# Title");
        assert!(result.contains("<h1>Title</h1>"));
    }

    #[test]
    fn renders_link() {
        let result = render_markdown("[click](https://example.com)");
        assert!(result.contains("<a"));
        assert!(result.contains("https://example.com"));
        assert!(result.contains("click"));
    }
}
