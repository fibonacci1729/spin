use crate::{Error, Result};

/// Template represents a simple string template that allows expressions in
/// double curly braces a la Mustache or Liquid.
#[derive(Debug, PartialEq)]
pub(crate) struct Template(Vec<Part>);

impl Template {
    pub(crate) fn new(template: impl Into<Box<str>>) -> Result<Self> {
        let mut parts = vec![];
        let mut remainder: Box<str> = template.into();
        while !remainder.is_empty() {
            let (part, rest) = if let Some(expr_rest) = remainder.strip_prefix("{{") {
                // Expression should be next
                if let Some((expr, rest)) = expr_rest.split_once("}}") {
                    // Take up through the next '}}'...
                    (Part::expr(expr.trim()), rest)
                } else {
                    // ...or we have unmatched braces
                    return Err(Error::InvalidTemplate(
                        "unmatched '{{' in template".to_string(),
                    ));
                }
            } else {
                // Literal is next
                if let Some(idx) = remainder.find("{{") {
                    // Take up to the next '{{'...
                    let (lit, rest) = remainder.split_at(idx);
                    (Part::lit(lit), rest)
                } else {
                    // ...or end of string
                    (Part::lit(remainder), "")
                }
            };
            parts.push(part);
            remainder = rest.into();
        }
        Ok(Template(parts))
    }

    pub(crate) fn parts(&self) -> impl Iterator<Item = &Part> {
        self.0.iter()
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum Part {
    Lit(Box<str>),
    Expr(Box<str>),
}

impl Part {
    pub fn lit(lit: impl Into<Box<str>>) -> Self {
        Self::Lit(lit.into())
    }

    pub fn expr(expr: impl Into<Box<str>>) -> Self {
        Self::Expr(expr.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parts() {
        for (tmpl, expected) in [
            ("", vec![]),
            ("a", vec![Part::lit("a")]),
            (
                "a-{{ expr }}-b",
                vec![Part::lit("a-"), Part::expr("expr"), Part::lit("-b")],
            ),
            (
                "{{ expr1 }}{{ expr2 }}",
                vec![Part::expr("expr1"), Part::expr("expr2")],
            ),
        ] {
            let template = Template::new(tmpl).unwrap();
            assert!(
                template.parts().eq(&expected),
                "{:?} -> {:?} != {:?}",
                tmpl,
                template,
                expected,
            );
        }
    }

    #[test]
    fn template_parts_bad() {
        Template::new("{{ matched }} {{ unmatched").unwrap_err();
    }
}
