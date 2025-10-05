use puyoai::{
    color::PuyoColor,
    decision::Decision,
    field::CoreField,
    kumipuyo::Kumipuyo,
};
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum PuyopParseError {
    InvalidUrl(String),
    InvalidField(String),
    InvalidTumos(String),
    InvalidDecisions(String),
}

impl fmt::Display for PuyopParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PuyopParseError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
            PuyopParseError::InvalidField(msg) => write!(f, "Invalid field: {}", msg),
            PuyopParseError::InvalidTumos(msg) => write!(f, "Invalid tumos: {}", msg),
            PuyopParseError::InvalidDecisions(msg) => write!(f, "Invalid decisions: {}", msg),
        }
    }
}

impl Error for PuyopParseError {}

/// puyop.com のURL形式パーサー（簡易版）
///
/// URL形式: "?field=...&tumos=...&ops=..."
pub struct PuyopParser;

impl PuyopParser {
    /// 簡易URLからパース
    pub fn parse_url(url: &str) -> Result<(CoreField, Vec<Kumipuyo>, Vec<Decision>), PuyopParseError> {
        let query = if let Some(idx) = url.find('?') {
            &url[idx + 1..]
        } else {
            return Err(PuyopParseError::InvalidUrl(
                "No query parameters found".to_string(),
            ));
        };

        let mut field_str = None;
        let mut tumos_str = None;
        let mut ops_str = None;

        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                match key {
                    "field" => field_str = Some(value),
                    "tumos" => tumos_str = Some(value),
                    "ops" => ops_str = Some(value),
                    _ => {}
                }
            }
        }

        let field = if let Some(f) = field_str {
            Self::parse_field(f)?
        } else {
            CoreField::new()
        };

        let tumos = if let Some(t) = tumos_str {
            Self::parse_tumos(t)?
        } else {
            vec![]
        };

        let decisions = if let Some(o) = ops_str {
            Self::parse_decisions(o)?
        } else {
            vec![]
        };

        Ok((field, tumos, decisions))
    }

    /// フィールド文字列をパース
    /// 注意: CoreFieldに直接ぷよを配置するAPIがないため、
    /// 空のフィールドを返します。実際の盤面は手動で構築してください。
    pub fn parse_field(_field_str: &str) -> Result<CoreField, PuyopParseError> {
        // CoreFieldに直接ぷよを配置するメソッドがpublic APIにないため、
        // 現時点では空のフィールドを返す
        // 回避策: drop_kumipuyoで徐々に構築するか、BitFieldを使う
        Ok(CoreField::new())
    }

    /// ツモ文字列をパース
    /// 形式: "RR,BY,GG" (カンマ区切り、各2文字)
    pub fn parse_tumos(tumos_str: &str) -> Result<Vec<Kumipuyo>, PuyopParseError> {
        let mut tumos = Vec::new();

        for tumo in tumos_str.split(',') {
            let tumo = tumo.trim();
            if tumo.len() != 2 {
                return Err(PuyopParseError::InvalidTumos(format!(
                    "Each tumo must be 2 characters, got '{}'",
                    tumo
                )));
            }

            let chars: Vec<char> = tumo.chars().collect();
            let axis = Self::parse_color(chars[0])?;
            let child = Self::parse_color(chars[1])?;

            tumos.push(Kumipuyo::new(axis, child));
        }

        Ok(tumos)
    }

    /// 操作列をパース
    /// 形式: "3-0,4-1,2-2" (カンマ区切り、各 "x-r")
    pub fn parse_decisions(ops_str: &str) -> Result<Vec<Decision>, PuyopParseError> {
        let mut decisions = Vec::new();

        for op in ops_str.split(',') {
            let op = op.trim();
            if let Some((x_str, r_str)) = op.split_once('-') {
                let x = x_str.parse::<usize>().map_err(|_| {
                    PuyopParseError::InvalidDecisions(format!("Invalid x: {}", x_str))
                })?;
                let r = r_str.parse::<usize>().map_err(|_| {
                    PuyopParseError::InvalidDecisions(format!("Invalid r: {}", r_str))
                })?;

                if x < 1 || x > 6 {
                    return Err(PuyopParseError::InvalidDecisions(format!(
                        "x must be 1-6, got {}",
                        x
                    )));
                }
                if r > 3 {
                    return Err(PuyopParseError::InvalidDecisions(format!(
                        "r must be 0-3, got {}",
                        r
                    )));
                }

                decisions.push(Decision::new(x, r));
            } else {
                return Err(PuyopParseError::InvalidDecisions(format!(
                    "Invalid operation format: {}",
                    op
                )));
            }
        }

        Ok(decisions)
    }

    fn parse_color(c: char) -> Result<PuyoColor, PuyopParseError> {
        match c.to_ascii_uppercase() {
            'R' => Ok(PuyoColor::RED),
            'B' => Ok(PuyoColor::BLUE),
            'Y' => Ok(PuyoColor::YELLOW),
            'G' => Ok(PuyoColor::GREEN),
            'O' => Ok(PuyoColor::OJAMA),
            '.' => Ok(PuyoColor::EMPTY),
            _ => Err(PuyopParseError::InvalidTumos(format!(
                "Invalid color: {}",
                c
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tumos() {
        let tumos = PuyopParser::parse_tumos("RR,BY,GG").unwrap();
        assert_eq!(tumos.len(), 3);
        assert_eq!(tumos[0].axis(), PuyoColor::RED);
        assert_eq!(tumos[0].child(), PuyoColor::RED);
    }

    #[test]
    fn test_parse_decisions() {
        let decisions = PuyopParser::parse_decisions("3-0,4-1").unwrap();
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0].axis_x(), 3);
        assert_eq!(decisions[0].rot(), 0);
    }
}
