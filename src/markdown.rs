use markdown::mdast::Node;
use markdown::{to_mdast, ParseOptions};
use textwrap::{fill, indent as prefix};

const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_ITALIC: &str = "\x1b[3m";

const ANSI_RESET: &str = "\x1b[0m";

/// Render markdown to manpage-style HTML.
pub fn markdown_to_html(source: &str) -> String {
    let rendered = ansi_to_html::convert_escaped(&markdown_to_ansi(source)).unwrap();
    format!("<code style=\"white-space: pre\">{rendered}</code>")
}

/// Render markdown to manpage-style ANSI-styled text.
pub fn markdown_to_ansi(source: &str) -> String {
    const FILL_WIDTH: usize = 80;
    const INDENT_AMOUNT: usize = 4;

    fn render_node(node: &Node) -> String {
        let mut level = 0;
        render_node_inner(node, &mut level)
    }

    fn render_nodes(nodes: &[Node]) -> String {
        let mut level = 0;
        nodes
            .iter()
            .map(|n| render_node_inner(n, &mut level))
            .fold(String::new(), |acc, s| acc + &s)
    }

    fn render_node_inner(node: &Node, level: &mut usize) -> String {
        let mut contents = match node {
            Node::Root(node) => render_nodes(&node.children),

            Node::Heading(node) => {
                // make subsequent nodes be at this 0-based level
                *level = node.depth as usize - 1;

                if *level == 0 {
                    // render top-level headings as unstyled center text
                    let inner = render_nodes(&node.children);
                    assert!(inner.len() <= FILL_WIDTH);
                    format!(
                        "{}{}\n\n",
                        " ".repeat((FILL_WIDTH - inner.len()) / 2 - 1),
                        inner
                    )
                } else {
                    // render all other headings as bold
                    indent_wrap(
                        &format!("{ANSI_BOLD}{}{ANSI_RESET}\n", render_nodes(&node.children)),
                        level.saturating_sub(1),
                    )
                }
            }

            Node::Text(node) => node.value.clone(),

            Node::Strong(node) => {
                format!("{ANSI_BOLD}{}{ANSI_RESET}", render_nodes(&node.children))
            }
            Node::Emphasis(node) => {
                format!("{ANSI_ITALIC}{}{ANSI_RESET}", render_nodes(&node.children))
            }
            Node::InlineCode(node) => format!("{}", node.value),

            Node::Link(node) => {
                let value = render_nodes(&node.children);
                if value == node.url {
                    node.url.clone()
                } else {
                    format!("{} ({})", value, node.url)
                }
            }

            Node::Paragraph(node) => indent_wrap(&render_nodes(&node.children), *level),
            Node::BlockQuote(node) => indent(&prefix(&render_nodes(&node.children), "> "), *level),
            Node::Code(node) => indent(&format!("{}", node.value), *level + 1),

            _ => unimplemented!("markdown feature not supported"),
        };

        let is_block = matches!(
            &node,
            Node::Paragraph(_) | Node::BlockQuote(_) | Node::Code(_)
        );
        if is_block {
            contents.push_str("\n\n");
        }

        contents
    }

    fn indent_wrap(text: &str, level: usize) -> String {
        indent(&fill(text, FILL_WIDTH - (INDENT_AMOUNT * level)), level)
    }

    fn indent(text: &str, level: usize) -> String {
        prefix(text, &" ".repeat(INDENT_AMOUNT * level))
    }

    // safe to unwrap when not using MDX
    let root = to_mdast(source, &ParseOptions::default()).unwrap();
    let body = render_node(&root);
    format!("{}\n", body.trim_end())
}
