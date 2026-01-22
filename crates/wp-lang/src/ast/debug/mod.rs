use std::io;

pub trait DebugFormat {
    fn fmt_string(&self) -> io::Result<String> {
        let w: Vec<u8> = vec![];
        let mut buf = PrettyFormatter {
            w,
            current_ident: 0,
        };
        self.write(&mut buf)?;

        Ok(unsafe { std::str::from_utf8_unchecked(&buf.w).to_string() })
    }

    fn write<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write + DepIndent;

    fn write_open_brace<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        write!(w, "{{")
    }

    fn write_close_brace<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        write!(w, "}}")
    }

    #[allow(dead_code)]
    fn write_open_square_bracket<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        write!(w, "[")
    }

    #[allow(dead_code)]
    fn write_close_square_bracket<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        write!(w, "]")
    }

    fn write_open_parenthesis<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        write!(w, "(")
    }

    fn write_close_parenthesis<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        write!(w, ")")
    }

    fn write_new_line<W>(&self, w: &mut W) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        writeln!(w)
    }

    fn default_indent(&self) -> &[u8] {
        b"  "
    }

    fn write_indent<W>(&self, w: &mut W, n: usize) -> io::Result<()>
    where
        W: ?Sized + io::Write,
    {
        for _ in 0..n {
            w.write_all(self.default_indent())?
        }
        Ok(())
    }
}

pub trait DepIndent {
    fn add_indent(&mut self) -> usize;
    fn sub_indent(&mut self) -> usize;
}

#[derive(Default)]
pub struct PrettyFormatter<W> {
    w: W,
    current_ident: usize,
}

impl<W> io::Write for PrettyFormatter<W>
where
    W: io::Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.w.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

impl<W> DepIndent for PrettyFormatter<W> {
    fn add_indent(&mut self) -> usize {
        self.current_ident += 1;
        self.current_ident
    }

    fn sub_indent(&mut self) -> usize {
        self.current_ident -= 1;
        self.current_ident
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::{Display, Formatter};
    use std::io::Write;

    use crate::ast::debug::{DebugFormat, DepIndent};
    use crate::parser::wpl_pkg::wpl_package;
    use orion_error::TestAssert;
    use wp_parser::Parser;

    #[test]
    fn test_debug_format() {
        struct Foo;

        impl Display for Foo {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.fmt_string().unwrap_or_default())
            }
        }
        impl DebugFormat for Foo {
            fn write<W>(&self, w: &mut W) -> std::io::Result<()>
            where
                W: ?Sized + Write + DepIndent,
            {
                // write hello{ a:[{}]}
                write!(w, "hello ")?;
                self.write_open_brace(w)?;
                self.write_new_line(w)?;
                let depth = w.add_indent();
                self.write_indent(w, depth)?;

                {
                    write!(w, "a: ")?;
                    self.write_open_square_bracket(w)?;
                    self.write_new_line(w)?;
                    let sec_depth = w.add_indent();
                    self.write_indent(w, sec_depth)?;
                    self.write_open_brace(w)?;
                    self.write_close_brace(w)?;
                    self.write_new_line(w)?;
                    w.sub_indent();
                    self.write_indent(w, depth)?;
                    self.write_close_square_bracket(w)?;
                }
                self.write_new_line(w)?;
                self.write_close_brace(w)?;
                Ok(())
            }
        }

        assert_eq!(
            Foo.to_string(),
            r#"hello {
  a: [
    {}
  ]
}"#
        );
    }

    #[test]
    fn test_format() {
        let code = r#"package /example {

   rule nginx {
        (ip:sip,2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_")
    }
}"#;
        let package = wpl_package.parse(code).assert();

        assert_eq!(
            package.to_string(),
            r#"package /example {
  rule nginx {
    (
      ip:sip,
      2*_,
      time<[,]>,
      http/request",
      http/status,
      digit,
      chars",
      http/agent",
      _"
    )
  }
}
"#
        );

        let code = r#"package /example {

   rule nginx {
        (ip:sip,symbol(<190>)[5],2*_,time<[,]>,http/request",http/status,digit,chars",http/agent",_")
    }
}"#;
        let package = wpl_package.parse(code).assert();

        assert_eq!(
            package.to_string(),
            r#"package /example {
  rule nginx {
    (
      ip:sip,
      symbol(<190>)[5],
      2*_,
      time<[,]>,
      http/request",
      http/status,
      digit,
      chars",
      http/agent",
      _"
    )
  }
}
"#
        );
    }
}
