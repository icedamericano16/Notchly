//! Quick maths, unit conversion and currency (free part of the palette).

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Op(char),
    LParen,
    RParen,
    Ident(String),
    Percent,
    Comma,
}

fn lex(s: &str) -> Option<Vec<Tok>> {
    let mut out = Vec::new();
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch.is_ascii_digit() || ch == '.' {
            let st = i;
            while i < c.len()
                && (c[i].is_ascii_digit()
                    || c[i] == '.'
                    || (c[i] == ','
                        && i + 3 < c.len() + 1
                        && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())
                        && c.get(i + 3).is_some_and(|d| d.is_ascii_digit())
                        && !c.get(i + 4).is_some_and(|d| d.is_ascii_digit())))
            {
                i += 1;
            }
            // scientific notation
            if i < c.len()
                && (c[i] == 'e' || c[i] == 'E')
                && c.get(i + 1)
                    .is_some_and(|d| d.is_ascii_digit() || *d == '-' || *d == '+')
            {
                i += 2;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let t: String = c[st..i].iter().filter(|x| **x != ',').collect();
            out.push(Tok::Num(t.parse().ok()?));
        } else if ch.is_alphabetic()
            || ch == '_'
            || ch == '°'
            || ch == '$'
            || ch == '€'
            || ch == '£'
            || ch == '₹'
        {
            let st = i;
            while i < c.len()
                && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '/' || c[i] == '°')
            {
                i += 1;
            }
            if i == st {
                i += 1;
            }
            out.push(Tok::Ident(
                c[st..i].iter().collect::<String>().to_lowercase(),
            ));
        } else {
            out.push(match ch {
                '+' | '-' | '*' | '/' | '^' => Tok::Op(ch),
                '×' | 'x' => Tok::Op('*'),
                '÷' => Tok::Op('/'),
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                '%' => Tok::Percent,
                ',' => Tok::Comma,
                _ => return None,
            });
            i += 1;
        }
    }
    Some(out)
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }
    // expr := term (('+'|'-') term)*   with "a + b%" meaning a + a*b/100
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?.0;
        while let Some(Tok::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.i += 1;
            let (r, pct) = self.term()?;
            let r = if pct { v * r } else { r };
            v = if op == '+' { v + r } else { v - r };
        }
        Some(v)
    }
    // returns (value, was_percent)
    fn term(&mut self) -> Option<(f64, bool)> {
        let (mut v, mut pct) = self.power()?;
        loop {
            match self.peek().cloned() {
                Some(Tok::Op(op @ ('*' | '/'))) => {
                    self.i += 1;
                    let (r, _) = self.power()?;
                    v = if op == '*' { v * r } else { v / r };
                    pct = false;
                }
                Some(Tok::Ident(w)) if w == "of" && pct => {
                    self.i += 1;
                    let (r, _) = self.power()?;
                    v *= r;
                    pct = false;
                }
                Some(Tok::Ident(w)) if w == "mod" => {
                    self.i += 1;
                    let (r, _) = self.power()?;
                    v %= r;
                    pct = false;
                }
                // implicit multiplication: 2(3+4), 2pi, 3 sqrt(4)
                Some(Tok::LParen) => {
                    let (r, _) = self.power()?;
                    v *= r;
                }
                Some(Tok::Ident(w)) if !matches!(w.as_str(), "of" | "mod" | "to" | "in" | "as") => {
                    let (r, _) = self.power()?;
                    v *= r;
                }
                _ => break,
            }
        }
        Some((v, pct))
    }
    fn power(&mut self) -> Option<(f64, bool)> {
        let (b, pct) = self.unary()?;
        if let Some(Tok::Op('^')) = self.peek() {
            self.i += 1;
            let (e, _) = self.power()?;
            return Some((b.powf(e), false));
        }
        Some((b, pct))
    }
    fn unary(&mut self) -> Option<(f64, bool)> {
        if let Some(Tok::Op('-')) = self.peek() {
            self.i += 1;
            let (v, p) = self.unary()?;
            return Some((-v, p));
        }
        if let Some(Tok::Op('+')) = self.peek() {
            self.i += 1;
        }
        let mut v = self.atom()?;
        let mut pct = false;
        if let Some(Tok::Percent) = self.peek() {
            self.i += 1;
            v /= 100.0;
            pct = true;
        }
        if let Some(Tok::Ident(w)) = self.peek() {
            if w == "!" {
                self.i += 1;
            }
        }
        Some((v, pct))
    }
    fn atom(&mut self) -> Option<f64> {
        match self.next()? {
            Tok::Num(n) => Some(n),
            Tok::LParen => {
                let v = self.expr()?;
                if let Some(Tok::RParen) = self.peek() {
                    self.i += 1;
                }
                Some(v)
            }
            Tok::Ident(name) => match name.as_str() {
                "pi" | "π" => Some(std::f64::consts::PI),
                "e" => Some(std::f64::consts::E),
                "tau" => Some(std::f64::consts::TAU),
                f => {
                    let arg = self.atom_arg()?;
                    Some(match f {
                        "sqrt" | "√" => arg.sqrt(),
                        "cbrt" => arg.cbrt(),
                        "sin" => arg.to_radians().sin(),
                        "cos" => arg.to_radians().cos(),
                        "tan" => arg.to_radians().tan(),
                        "asin" => arg.asin().to_degrees(),
                        "acos" => arg.acos().to_degrees(),
                        "atan" => arg.atan().to_degrees(),
                        "log" => arg.log10(),
                        "ln" => arg.ln(),
                        "log2" => arg.log2(),
                        "exp" => arg.exp(),
                        "abs" => arg.abs(),
                        "round" => arg.round(),
                        "floor" => arg.floor(),
                        "ceil" => arg.ceil(),
                        _ => return None,
                    })
                }
            },
            _ => None,
        }
    }
    fn atom_arg(&mut self) -> Option<f64> {
        if let Some(Tok::LParen) = self.peek() {
            self.i += 1;
            let v = self.expr()?;
            if let Some(Tok::RParen) = self.peek() {
                self.i += 1;
            }
            Some(v)
        } else {
            self.unary().map(|v| v.0)
        }
    }
}

/// Evaluates a maths expression. Returns None unless the whole input parses.
pub fn eval(s: &str) -> Option<f64> {
    let t = lex(s)?;
    if t.is_empty()
        || !t.iter().any(|x| {
            matches!(x, Tok::Num(_)) || matches!(x, Tok::Ident(n) if n == "pi" || n == "e")
        })
    {
        return None;
    }
    let mut p = Parser { t, i: 0 };
    let v = p.expr()?;
    if p.i < p.t.len() || !v.is_finite() {
        return None;
    }
    Some(v)
}

pub fn fmt_num(v: f64) -> String {
    if v.abs() >= 1e15 || (v != 0.0 && v.abs() < 1e-6) {
        return format!("{v:e}");
    }
    let r = (v * 1e10).round() / 1e10;
    let mut s = format!("{r:.10}");
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    // thousands separators on the integer part
    let (int, frac) = match s.split_once('.') {
        Some((a, b)) => (a.to_string(), Some(b.to_string())),
        None => (s.clone(), None),
    };
    let neg = int.starts_with('-');
    let digits: Vec<char> = int.trim_start_matches('-').chars().collect();
    let mut grouped = String::new();
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(*c);
    }
    format!(
        "{}{}{}",
        if neg { "-" } else { "" },
        grouped,
        frac.map(|f| format!(".{f}")).unwrap_or_default()
    )
}

// ---------------------------------------------------------------- units

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cat {
    Length,
    Mass,
    Temp,
    Data,
    Time,
    Speed,
    Area,
    Volume,
}

fn unit(name: &str) -> Option<(Cat, f64, &'static str)> {
    use Cat::*;
    Some(match name.trim_end_matches('.') {
        "mm" | "millimeter" | "millimeters" | "millimetre" => (Length, 0.001, "mm"),
        "cm" | "centimeter" | "centimeters" | "centimetre" => (Length, 0.01, "cm"),
        "m" | "meter" | "meters" | "metre" | "metres" => (Length, 1.0, "m"),
        "km" | "kilometer" | "kilometers" | "kilometre" => (Length, 1000.0, "km"),
        "in" | "inch" | "inches" => (Length, 0.0254, "in"),
        "ft" | "foot" | "feet" => (Length, 0.3048, "ft"),
        "yd" | "yard" | "yards" => (Length, 0.9144, "yd"),
        "mi" | "mile" | "miles" => (Length, 1609.344, "mi"),
        "nmi" => (Length, 1852.0, "nmi"),
        "mg" | "milligram" | "milligrams" => (Mass, 0.001, "mg"),
        "g" | "gram" | "grams" => (Mass, 1.0, "g"),
        "kg" | "kilo" | "kilos" | "kilogram" | "kilograms" => (Mass, 1000.0, "kg"),
        "t" | "tonne" | "tonnes" => (Mass, 1_000_000.0, "t"),
        "oz" | "ounce" | "ounces" => (Mass, 28.349523125, "oz"),
        "lb" | "lbs" | "pound" | "pounds" => (Mass, 453.59237, "lb"),
        "st" | "stone" => (Mass, 6350.29318, "st"),
        "tola" => (Mass, 11.6638, "tola"),
        "c" | "°c" | "celsius" => (Temp, 0.0, "°C"),
        "f" | "°f" | "fahrenheit" => (Temp, 1.0, "°F"),
        "k" | "kelvin" => (Temp, 2.0, "K"),
        "b" | "byte" | "bytes" => (Data, 1.0, "B"),
        "kb" | "kilobyte" | "kilobytes" => (Data, 1e3, "kB"),
        "mb" | "megabyte" | "megabytes" => (Data, 1e6, "MB"),
        "gb" | "gigabyte" | "gigabytes" => (Data, 1e9, "GB"),
        "tb" | "terabyte" | "terabytes" => (Data, 1e12, "TB"),
        "kib" => (Data, 1024.0, "KiB"),
        "mib" => (Data, 1_048_576.0, "MiB"),
        "gib" => (Data, 1_073_741_824.0, "GiB"),
        "tib" => (Data, 1_099_511_627_776.0, "TiB"),
        "bit" | "bits" => (Data, 0.125, "bit"),
        "ms" | "millisecond" | "milliseconds" => (Time, 0.001, "ms"),
        "s" | "sec" | "secs" | "second" | "seconds" => (Time, 1.0, "s"),
        "min" | "mins" | "minute" | "minutes" => (Time, 60.0, "min"),
        "h" | "hr" | "hrs" | "hour" | "hours" => (Time, 3600.0, "h"),
        "d" | "day" | "days" => (Time, 86400.0, "d"),
        "wk" | "week" | "weeks" => (Time, 604800.0, "wk"),
        "yr" | "year" | "years" => (Time, 31_557_600.0, "yr"),
        "m/s" | "mps" => (Speed, 1.0, "m/s"),
        "km/h" | "kmh" | "kph" => (Speed, 1.0 / 3.6, "km/h"),
        "mph" => (Speed, 0.44704, "mph"),
        "knot" | "knots" | "kn" => (Speed, 0.514444, "kn"),
        "m2" | "sqm" => (Area, 1.0, "m²"),
        "km2" => (Area, 1e6, "km²"),
        "ft2" | "sqft" => (Area, 0.09290304, "ft²"),
        "acre" | "acres" => (Area, 4046.8564224, "acre"),
        "ha" | "hectare" | "hectares" => (Area, 10000.0, "ha"),
        "ropani" => (Area, 508.737, "ropani"),
        "ml" | "milliliter" | "milliliters" => (Volume, 0.001, "mL"),
        "l" | "liter" | "liters" | "litre" | "litres" => (Volume, 1.0, "L"),
        "gal" | "gallon" | "gallons" => (Volume, 3.785411784, "gal"),
        "cup" | "cups" => (Volume, 0.2365882365, "cup"),
        "floz" => (Volume, 0.0295735295625, "fl oz"),
        _ => return None,
    })
}

fn convert_temp(v: f64, from: f64, to: f64) -> f64 {
    let c = match from as i32 {
        0 => v,
        1 => (v - 32.0) * 5.0 / 9.0,
        _ => v - 273.15,
    };
    match to as i32 {
        0 => c,
        1 => c * 9.0 / 5.0 + 32.0,
        _ => c + 273.15,
    }
}

/// Currency rates relative to EUR.
pub type Rates = HashMap<String, f64>;

/// Splits "<expr> <unit> (to|in|as) <unit>".
fn split_conversion(s: &str) -> Option<(String, String, String)> {
    let l = s.to_lowercase();
    for sep in [" to ", " in ", " as ", " => ", " -> "] {
        if let Some(pos) = l.rfind(sep) {
            let (lhs, rhs) = (l[..pos].trim(), l[pos + sep.len()..].trim());
            let rhs = rhs.to_string();
            // lhs ends with a unit token
            let lhs = lhs
                .replace(['$'], " usd ")
                .replace('€', " eur ")
                .replace('£', " gbp ")
                .replace('₹', " inr ");
            let lhs = lhs.trim();
            // "usd 10" (symbol before the amount)
            let parts: Vec<&str> = lhs.split_whitespace().collect();
            if parts.len() == 2
                && parts[0].chars().all(|c| c.is_alphabetic())
                && eval(parts[1]).is_some()
            {
                return Some((parts[1].to_string(), parts[0].to_string(), rhs));
            }
            let split_at = lhs
                .rfind(|c: char| c.is_ascii_digit() || c == ')' || c == ' ')
                .map(|i| i + 1)
                .unwrap_or(0);
            let (num, u) = lhs.split_at(split_at);
            let (num, u) = (num.trim(), u.trim());
            if u.is_empty() {
                // "100 usd" with a space: the unit is the last word
                let mut parts: Vec<&str> = lhs.split_whitespace().collect();
                let u = parts.pop()?.to_string();
                return Some((parts.join(" "), u, rhs));
            }
            return Some((num.to_string(), u.to_string(), rhs));
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct Answer {
    pub value: f64,
    pub text: String,
}

/// Evaluates maths, unit conversion or currency conversion.
pub fn evaluate(input: &str, rates: &Rates) -> Option<Answer> {
    let s = input.trim().trim_start_matches('=').trim();
    if s.is_empty() {
        return None;
    }
    if let Some((num, from, to)) = split_conversion(s) {
        let v = if num.is_empty() {
            Some(1.0)
        } else {
            eval(&num)
        };
        if let Some(v) = v {
            if let (Some((c1, f1, _)), Some((c2, f2, n2))) = (unit(&from), unit(&to)) {
                if c1 == c2 {
                    let r = if c1 == Cat::Temp {
                        convert_temp(v, f1, f2)
                    } else {
                        v * f1 / f2
                    };
                    return Some(Answer {
                        value: r,
                        text: format!("{} {}", fmt_num(r), n2),
                    });
                }
            }
            let (fu, tu) = (from.to_uppercase(), to.to_uppercase());
            if let (Some(a), Some(b)) = (rate(rates, &fu), rate(rates, &tu)) {
                let r = v / a * b;
                return Some(Answer {
                    value: r,
                    text: format!("{} {}", fmt_money(r), tu),
                });
            }
        }
    }
    let v = eval(s)?;
    Some(Answer {
        value: v,
        text: fmt_num(v),
    })
}

fn rate(rates: &Rates, code: &str) -> Option<f64> {
    if code == "EUR" {
        return Some(1.0);
    }
    if let Some(r) = rates.get(code) {
        return Some(*r);
    }
    // NPR is pegged to INR at 1.6
    if code == "NPR" {
        return rates.get("INR").map(|r| r * 1.6);
    }
    None
}

fn fmt_money(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    fmt_num(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arithmetic() {
        assert_eq!(eval("1 + 2 * 3"), Some(7.0));
        assert_eq!(eval("(1 + 2) * 3"), Some(9.0));
        assert_eq!(eval("2^3^2"), Some(512.0));
        assert_eq!(eval("-3 + 5"), Some(2.0));
        assert_eq!(eval("10 ÷ 4"), Some(2.5));
        assert_eq!(eval("3 × 4"), Some(12.0));
        assert_eq!(eval("1,000 + 1"), Some(1001.0));
        assert_eq!(eval("2(3+4)"), Some(14.0));
    }
    #[test]
    fn percent() {
        assert_eq!(eval("20% of 150"), Some(30.0));
        assert_eq!(eval("200 + 10%"), Some(220.0));
        assert_eq!(eval("50%"), Some(0.5));
    }
    #[test]
    fn functions() {
        assert!((eval("sqrt(16)").unwrap() - 4.0).abs() < 1e-9);
        assert!((eval("sin(30)").unwrap() - 0.5).abs() < 1e-9);
        assert!((eval("2pi").unwrap() - std::f64::consts::TAU).abs() < 1e-9);
        assert_eq!(eval("log(1000)"), Some(3.0));
    }
    #[test]
    fn rejects_text() {
        assert_eq!(eval("hello world"), None);
        assert_eq!(eval("2 +"), None);
    }
    #[test]
    fn units() {
        let r = Rates::new();
        assert_eq!(evaluate("5 km to m", &r).unwrap().value, 5000.0);
        assert!((evaluate("100 f to c", &r).unwrap().value - 37.7777).abs() < 0.01);
        assert!((evaluate("1 mi in km", &r).unwrap().value - 1.609344).abs() < 1e-9);
        assert_eq!(evaluate("2 gb to mb", &r).unwrap().value, 2000.0);
        assert!((evaluate("60 mph to km/h", &r).unwrap().value - 96.56).abs() < 0.01);
    }
    #[test]
    fn currency() {
        let mut r = Rates::new();
        r.insert("USD".into(), 1.1);
        r.insert("INR".into(), 92.0);
        let a = evaluate("110 usd to eur", &r).unwrap();
        assert!((a.value - 100.0).abs() < 1e-9);
        let n = evaluate("1 eur to npr", &r).unwrap();
        assert!((n.value - 147.2).abs() < 1e-9);
        assert!(evaluate("$10 to inr", &r).is_some());
    }
    #[test]
    fn formats() {
        assert_eq!(fmt_num(1234567.5), "1,234,567.5");
        assert_eq!(fmt_num(0.1 + 0.2), "0.3");
        assert_eq!(fmt_num(-1000.0), "-1,000");
    }
}
