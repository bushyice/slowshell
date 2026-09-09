use std::f64::consts::{E, PI};

#[derive(Debug, PartialEq, Clone)]
enum Token {
  Num(f64),
  Ident(String),
  Plus,
  Minus,
  Star,
  Slash,
  Percent,
  Caret,
  LParen,
  RParen,
}

struct Lexer<'a> {
  chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Lexer<'a> {
  fn new(input: &'a str) -> Self {
    Self {
      chars: input.chars().peekable(),
    }
  }

  fn tokenize(&mut self) -> Result<Vec<Token>, ()> {
    let mut tokens = Vec::new();

    while let Some(&c) = self.chars.peek() {
      match c {
        ' ' | '\t' | '\r' | '\n' => {
          self.chars.next();
        }
        '0'..='9' | '.' => {
          let mut s = String::new();
          let mut has_dot = false;
          while let Some(&ch) = self.chars.peek() {
            if ch == '.' {
              if has_dot {
                return Err(());
              }
              has_dot = true;
              s.push(self.chars.next().unwrap());
            } else if ch.is_ascii_digit() {
              s.push(self.chars.next().unwrap());
            } else {
              break;
            }
          }
          let n = s.parse::<f64>().map_err(|_| ())?;
          tokens.push(Token::Num(n));
        }
        'a'..='z' | 'A'..='Z' | '_' => {
          let mut s = String::new();
          while let Some(&ch) = self.chars.peek() {
            if ch.is_alphanumeric() || ch == '_' {
              s.push(self.chars.next().unwrap().to_ascii_lowercase());
            } else {
              break;
            }
          }
          tokens.push(Token::Ident(s));
        }
        '+' => {
          self.chars.next();
          tokens.push(Token::Plus);
        }
        '-' => {
          self.chars.next();
          tokens.push(Token::Minus);
        }
        '*' => {
          self.chars.next();
          tokens.push(Token::Star);
        }
        '/' => {
          self.chars.next();
          tokens.push(Token::Slash);
        }
        '%' => {
          self.chars.next();
          tokens.push(Token::Percent);
        }
        '^' => {
          self.chars.next();
          tokens.push(Token::Caret);
        }
        '(' => {
          self.chars.next();
          tokens.push(Token::LParen);
        }
        ')' => {
          self.chars.next();
          tokens.push(Token::RParen);
        }
        _ => return Err(()),
      }
    }

    Ok(tokens)
  }
}

struct Parser {
  tokens: Vec<Token>,
  pos: usize,
}

impl Parser {
  fn new(tokens: Vec<Token>) -> Self {
    Self { tokens, pos: 0 }
  }

  fn peek(&self) -> Option<&Token> {
    self.tokens.get(self.pos)
  }

  fn next(&mut self) -> Option<Token> {
    if self.pos < self.tokens.len() {
      let tok = self.tokens[self.pos].clone();
      self.pos += 1;
      Some(tok)
    } else {
      None
    }
  }

  fn parse_expr(&mut self) -> Result<f64, ()> {
    self.parse_add_sub()
  }

  fn parse_add_sub(&mut self) -> Result<f64, ()> {
    let mut left = self.parse_mul_div()?;

    while let Some(tok) = self.peek() {
      match tok {
        Token::Plus => {
          self.next();
          let right = self.parse_mul_div()?;
          left += right;
        }
        Token::Minus => {
          self.next();
          let right = self.parse_mul_div()?;
          left -= right;
        }
        _ => break,
      }
    }

    Ok(left)
  }

  fn parse_mul_div(&mut self) -> Result<f64, ()> {
    let mut left = self.parse_power()?;

    while let Some(tok) = self.peek() {
      match tok {
        Token::Star => {
          self.next();
          let right = self.parse_power()?;
          left *= right;
        }
        Token::Slash => {
          self.next();
          let right = self.parse_power()?;
          if right == 0.0 {
            return Err(());
          }
          left /= right;
        }
        Token::Percent => {
          self.next();
          let right = self.parse_power()?;
          if right == 0.0 {
            return Err(());
          }
          left %= right;
        }
        _ => break,
      }
    }

    Ok(left)
  }

  fn parse_power(&mut self) -> Result<f64, ()> {
    let base = self.parse_unary()?;

    if let Some(Token::Caret) = self.peek() {
      self.next();
      let exponent = self.parse_power()?;
      Ok(base.powf(exponent))
    } else {
      Ok(base)
    }
  }

  fn parse_unary(&mut self) -> Result<f64, ()> {
    match self.peek() {
      Some(Token::Plus) => {
        self.next();
        self.parse_unary()
      }
      Some(Token::Minus) => {
        self.next();
        Ok(-self.parse_unary()?)
      }
      _ => self.parse_primary(),
    }
  }

  fn parse_primary(&mut self) -> Result<f64, ()> {
    match self.next() {
      Some(Token::Num(n)) => Ok(n),
      Some(Token::Ident(name)) => match name.as_str() {
        "pi" => Ok(PI),
        "e" => Ok(E),
        func @ ("sqrt" | "cbrt" | "abs" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan"
        | "log" | "ln" | "log2" | "round" | "floor" | "ceil") => {
          if let Some(Token::LParen) = self.next() {
            let arg = self.parse_expr()?;
            if let Some(Token::RParen) = self.next() {
              match func {
                "sqrt" => {
                  if arg < 0.0 {
                    Err(())
                  } else {
                    Ok(arg.sqrt())
                  }
                }
                "cbrt" => Ok(arg.cbrt()),
                "abs" => Ok(arg.abs()),
                "sin" => Ok(arg.sin()),
                "cos" => Ok(arg.cos()),
                "tan" => Ok(arg.tan()),
                "asin" => {
                  if (-1.0..=1.0).contains(&arg) {
                    Ok(arg.asin())
                  } else {
                    Err(())
                  }
                }
                "acos" => {
                  if (-1.0..=1.0).contains(&arg) {
                    Ok(arg.acos())
                  } else {
                    Err(())
                  }
                }
                "atan" => Ok(arg.atan()),
                "log" => {
                  if arg <= 0.0 {
                    Err(())
                  } else {
                    Ok(arg.log10())
                  }
                }
                "ln" => {
                  if arg <= 0.0 {
                    Err(())
                  } else {
                    Ok(arg.ln())
                  }
                }
                "log2" => {
                  if arg <= 0.0 {
                    Err(())
                  } else {
                    Ok(arg.log2())
                  }
                }
                "round" => Ok(arg.round()),
                "floor" => Ok(arg.floor()),
                "ceil" => Ok(arg.ceil()),
                _ => Err(()),
              }
            } else {
              Err(())
            }
          } else {
            Err(())
          }
        }
        _ => Err(()),
      },
      Some(Token::LParen) => {
        let val = self.parse_expr()?;
        if let Some(Token::RParen) = self.next() {
          Ok(val)
        } else {
          Err(())
        }
      }
      _ => Err(()),
    }
  }
}

pub fn eval(input: &str) -> Result<f64, ()> {
  let trimmed = input.trim();
  if trimmed.is_empty() {
    return Err(());
  }

  let mut lexer = Lexer::new(trimmed);
  let tokens = lexer.tokenize()?;
  if tokens.is_empty() {
    return Err(());
  }

  let mut parser = Parser::new(tokens);
  let result = parser.parse_expr()?;

  if parser.pos != parser.tokens.len() {
    return Err(());
  }

  if result.is_nan() || result.is_infinite() {
    return Err(());
  }

  Ok(result)
}

pub fn is_math_expression(input: &str) -> bool {
  let trimmed = input.trim();
  if trimmed.is_empty() {
    return false;
  }

  let has_operator = trimmed
    .chars()
    .any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^'));
  let has_func = trimmed.contains("sqrt")
    || trimmed.contains("cbrt")
    || trimmed.contains("abs")
    || trimmed.contains("sin")
    || trimmed.contains("cos")
    || trimmed.contains("tan")
    || trimmed.contains("log")
    || trimmed.contains("ln")
    || trimmed.contains("pi");

  if !has_operator && !has_func {
    return false;
  }

  eval(trimmed).is_ok()
}

pub fn format_result(val: f64) -> String {
  if val.fract() == 0.0 && val.abs() < 1e15 {
    format!("{:.0}", val)
  } else {
    let s = format!("{:.6}", val);
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_string()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn basic_arithmetic() {
    assert_eq!(eval("2 + 2"), Ok(4.0));
    assert_eq!(eval("10 - 3 * 2"), Ok(4.0));
    assert_eq!(eval("(10 - 3) * 2"), Ok(14.0));
    assert_eq!(eval("2 ^ 3"), Ok(8.0));
    assert_eq!(eval("10 / 4"), Ok(2.5));
    assert_eq!(eval("10 % 3"), Ok(1.0));
  }

  #[test]
  fn functions() {
    assert_eq!(eval("sqrt(16) + 2"), Ok(6.0));
    assert_eq!(eval("abs(-42)"), Ok(42.0));
  }

  #[test]
  fn trigger_check() {
    assert!(is_math_expression("2 + 2"));
    assert!(is_math_expression("sqrt(16)"));
    assert!(is_math_expression("10 * 3.5"));
    assert!(!is_math_expression("firefox"));
    assert!(!is_math_expression("2"));
    assert!(!is_math_expression("2 +"));
  }
}
