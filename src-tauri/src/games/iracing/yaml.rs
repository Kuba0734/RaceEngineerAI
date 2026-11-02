//! Minimal reader for iRacing's session info YAML: block maps, block lists ("- " items,
//! also at the same indent as their key) and plain or quoted scalars. That is all the sim
//! writes. A full YAML parser is not used on purpose: values such as driver and team names
//! are written unescaped and would break a strict parser.

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Scalar(String),
    Map(Vec<(String, Node)>),
    List(Vec<Node>),
}

struct Line<'a> {
    indent: usize,
    item: bool,
    text: &'a str,
}

pub fn parse(src: &str) -> Node {
    let lines: Vec<Line> = src
        .lines()
        .filter_map(|l| {
            let t = l.trim_end();
            let body = t.trim_start();
            if body.is_empty() || body == "---" || body == "..." || body.starts_with('#') {
                return None;
            }
            let indent = t.len() - body.len();
            match body.strip_prefix("- ").or(if body == "-" { Some("") } else { None }) {
                Some(rest) => Some(Line { indent, item: true, text: rest }),
                None => Some(Line { indent, item: false, text: body }),
            }
        })
        .collect();
    let mut p = Parser { lines, i: 0 };
    p.block(0)
}

struct Parser<'a> {
    lines: Vec<Line<'a>>,
    i: usize,
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    let q = v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')));
    if q {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

/// "Key: value" → (key, value); "Key:" → (key, ""). None for plain scalars.
fn split_key(text: &str) -> Option<(&str, &str)> {
    if let Some(k) = text.strip_suffix(':') {
        return Some((k.trim(), ""));
    }
    text.split_once(": ").map(|(k, v)| (k.trim(), v.trim()))
}

impl Parser<'_> {
    fn block(&mut self, min_indent: usize) -> Node {
        let Some(l) = self.lines.get(self.i) else { return Node::Map(Vec::new()) };
        if l.indent < min_indent {
            return Node::Map(Vec::new());
        }
        let indent = l.indent;
        if l.item {
            self.list(indent)
        } else {
            self.map(indent)
        }
    }

    fn map(&mut self, indent: usize) -> Node {
        let mut out = Vec::new();
        while let Some(l) = self.lines.get(self.i) {
            if l.indent != indent || l.item {
                break;
            }
            let text = l.text;
            self.i += 1;
            let Some((k, v)) = split_key(text) else { continue };
            let value = if !v.is_empty() {
                Node::Scalar(unquote(v))
            } else {
                match self.lines.get(self.i) {
                    Some(n) if n.indent > indent => self.block(indent + 1),
                    // Lists may sit at the same indent as their key.
                    Some(n) if n.indent == indent && n.item => self.list(indent),
                    _ => Node::Scalar(String::new()),
                }
            };
            out.push((k.to_string(), value));
        }
        Node::Map(out)
    }

    fn list(&mut self, indent: usize) -> Node {
        let mut out = Vec::new();
        while let Some(l) = self.lines.get(self.i) {
            if l.indent != indent || !l.item {
                break;
            }
            // The item's content starts after "- " and continues at that column.
            let inner = indent + 2;
            if split_key(l.text).is_none() {
                out.push(Node::Scalar(unquote(l.text)));
                self.i += 1;
                continue;
            }
            self.lines[self.i] = Line { indent: inner, item: false, text: self.lines[self.i].text };
            out.push(self.map(inner));
        }
        Node::List(out)
    }
}

impl Node {
    /// Path lookup: "WeekendInfo.TrackName".
    pub fn get(&self, path: &str) -> Option<&Node> {
        path.split('.').try_fold(self, |n, k| match n {
            Node::Map(m) => m.iter().find(|(key, _)| key == k).map(|(_, v)| v),
            _ => None,
        })
    }

    pub fn str(&self, path: &str) -> Option<&str> {
        match self.get(path)? {
            Node::Scalar(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Leading number of a scalar: "3.98 km" → 3.98, "300.00 sec" → 300.
    pub fn num(&self, path: &str) -> Option<f64> {
        leading_number(self.str(path)?)
    }

    pub fn list(&self, path: &str) -> &[Node] {
        match self.get(path) {
            Some(Node::List(l)) => l,
            _ => &[],
        }
    }
}

pub fn leading_number(s: &str) -> Option<f64> {
    let s = s.trim();
    let end = s.char_indices().find(|(i, c)| !(c.is_ascii_digit() || *c == '.' || ((*c == '-' || *c == '+') && *i == 0))).map(|(i, _)| i).unwrap_or(s.len());
    s[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---
WeekendInfo:
 TrackName: sonoma long
 TrackLength: 3.98 km
 TrackDisplayName: Sonoma Raceway
 WeekendOptions:
  NumStarters: 0
SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionLaps: unlimited
   SessionTime: 300.00 sec
   SessionType: Practice
   ResultsPositions:
 - SessionNum: 1
   SessionLaps: 14
   SessionTime: unlimited
   SessionType: Race
   ResultsPositions:
   - Position: 1
     CarIdx: 3
DriverInfo:
 DriverCarIdx: 1
 Drivers:
 - CarIdx: 0
   UserName: Pace Car
   CarNumber: \"0\"
 - CarIdx: 1
   UserName: Jan Kowalski: Jr.
   TeamName: 'Team #1 – Ölwechsel'
   CarNumber: \"07\"
SplitTimeInfo:
 Sectors:
 - SectorNum: 0
   SectorStartPct: 0.000000
 - SectorNum: 1
   SectorStartPct: 0.412000
...
";

    #[test]
    fn parses_iracing_session_yaml() {
        let y = parse(SAMPLE);
        assert_eq!(y.str("WeekendInfo.TrackName"), Some("sonoma long"));
        assert_eq!(y.num("WeekendInfo.TrackLength"), Some(3.98));
        assert_eq!(y.num("WeekendInfo.WeekendOptions.NumStarters"), Some(0.0));
        let s = y.list("SessionInfo.Sessions");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].str("SessionLaps"), Some("unlimited"));
        assert_eq!(s[0].num("SessionTime"), Some(300.0));
        assert_eq!(s[1].str("SessionType"), Some("Race"));
        assert_eq!(s[1].list("ResultsPositions")[0].num("CarIdx"), Some(3.0));
        assert!(s[0].list("ResultsPositions").is_empty());
        let d = y.list("DriverInfo.Drivers");
        assert_eq!(d.len(), 2);
        assert_eq!(d[1].str("UserName"), Some("Jan Kowalski: Jr."));
        assert_eq!(d[1].str("TeamName"), Some("Team #1 – Ölwechsel"));
        assert_eq!(d[1].str("CarNumber"), Some("07"));
        assert_eq!(y.num("DriverInfo.DriverCarIdx"), Some(1.0));
        assert_eq!(y.list("SplitTimeInfo.Sectors")[1].num("SectorStartPct"), Some(0.412));
    }

    #[test]
    fn numbers() {
        assert_eq!(leading_number("-1.5 C"), Some(-1.5));
        assert_eq!(leading_number("unlimited"), None);
    }
}
