use puyoai::{
    color::PuyoColor,
    decision::Decision,
    field::CoreField,
    kumipuyo::Kumipuyo,
};
use std::collections::HashMap;

/// puyop.com URL デコーダー
///
/// エンコーディング仕様:
/// - URL形式: http://www.puyop.com/s/{field}_{control}
/// - field: 盤面を3列ずつペアでエンコード（13段×3ペア＝39文字）
/// - control: ツモと操作を2文字ずつエンコード
pub struct PuyopDecoder {
    decoder_map: HashMap<char, usize>,
}

impl PuyopDecoder {
    const ENCODER: &'static [char] = &[
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j',
        'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't',
        'u', 'v', 'w', 'x', 'y', 'z',
        'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J',
        'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T',
        'U', 'V', 'W', 'X', 'Y', 'Z',
        '[', ']',
    ];

    pub fn new() -> Self {
        let mut decoder_map = HashMap::new();
        for (i, &c) in Self::ENCODER.iter().enumerate() {
            decoder_map.insert(c, i);
        }
        PuyopDecoder { decoder_map }
    }

    /// puyop.comのURLから盤面、ツモ、操作をデコード
    ///
    /// URL形式:
    /// - http://www.puyop.com/s/{encoded}
    /// - https://puyop.com/s/{encoded}
    /// - {encoded} = {field} または {field}_{control}
    pub fn decode_url(&self, url: &str) -> Result<(CoreField, Vec<Kumipuyo>, Vec<Decision>), String> {
        // URLからエンコード部分を抽出
        let encoded = if let Some(idx) = url.rfind("/s/") {
            &url[idx + 3..]
        } else if url.starts_with("http") {
            return Err("Invalid puyop.com URL format".to_string());
        } else {
            // URLプレフィックスなしの場合、そのままエンコード文字列として扱う
            url
        };

        // field と control を分離
        let (field_part, control_part) = if let Some(idx) = encoded.find('_') {
            (&encoded[..idx], Some(&encoded[idx + 1..]))
        } else {
            (encoded, None)
        };

        // フィールドをデコード
        let field = self.decode_field(field_part)?;

        // コントロール部分をデコード
        let (seq, decisions) = if let Some(ctrl) = control_part {
            self.decode_control(ctrl)?
        } else {
            (vec![], vec![])
        };

        Ok((field, seq, decisions))
    }

    /// フィールド部分をデコード
    ///
    /// エンコーディング:
    /// - 3列ペア (1-2, 3-4, 5-6) を1文字にエンコード
    /// - 上から下へ (y=13 → y=1)
    /// - d = color(px) * 8 + color(px+1)  ※pxは1,3,5
    fn decode_field(&self, encoded: &str) -> Result<CoreField, String> {
        if encoded.is_empty() {
            return Ok(CoreField::new());
        }

        let chars: Vec<char> = encoded.chars().collect();

        // エンコーダーのスキャン順序：y=13→1, 各yでpx=1,3,5
        // 最初の非空ペア以降、すべてのペアがエンコードされている
        // まず各エンコード文字がどの(y,px)に対応するか計算

        // エンコーダーは最初の非空ペアから開始し、その後すべてのペアを出力
        // 開始位置は (文字数 + offset) % 3 == 0 となるoffsetから逆算できる
        // offset = 0 (px=1で開始), 1 (px=3で開始), 2 (px=5で開始)
        let mut start_px_idx = 0;
        for offset in 0..3 {
            if (chars.len() + offset) % 3 == 0 {
                start_px_idx = offset;
                break;
            }
        }

        // 行を構築
        let mut rows = vec![];
        let mut current_row = vec!['.'; 6];
        let mut px_idx = start_px_idx;

        for &c in chars.iter() {
            let d = self.decoder_map.get(&c)
                .ok_or_else(|| format!("Invalid character in field: {}", c))?;

            let color_left = Self::field_id_to_color((d / 8) as usize);
            let color_right = Self::field_id_to_color((d % 8) as usize);

            let px = [1, 3, 5][px_idx];
            current_row[px - 1] = Self::color_to_char(color_left);
            current_row[px] = Self::color_to_char(color_right);

            px_idx += 1;
            if px_idx >= 3 {
                rows.push(current_row.clone());
                current_row = vec!['.'; 6];
                px_idx = 0;
            }
        }

        // 最後の不完全な行を追加
        if px_idx > 0 {
            rows.push(current_row);
        }

        let field_str: String = rows.iter()
            .map(|row| row.iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("");

        // CoreField::from_str() を使って構築
        Ok(CoreField::from_str(&field_str))
    }

    fn color_to_char(color: PuyoColor) -> char {
        match color {
            PuyoColor::EMPTY => '.',
            PuyoColor::RED => 'R',
            PuyoColor::GREEN => 'G',
            PuyoColor::BLUE => 'B',
            PuyoColor::YELLOW => 'Y',
            PuyoColor::OJAMA => 'O',
            _ => '.',
        }
    }

    /// コントロール部分をデコード
    ///
    /// エンコーディング:
    /// - 各手は2文字
    /// - d0 = (tsumo_axis * 5 + tsumo_child) | ((axis_x << 2 | rot) << 7)
    /// - 1文字目: d0 & 0x3F
    /// - 2文字目: (d0 >> 6) & 0x3F
    fn decode_control(&self, encoded: &str) -> Result<(Vec<Kumipuyo>, Vec<Decision>), String> {
        let chars: Vec<char> = encoded.chars().collect();
        if chars.len() % 2 != 0 {
            return Err("Control part must have even number of characters".to_string());
        }

        let mut seq = Vec::new();
        let mut decisions = Vec::new();

        for i in (0..chars.len()).step_by(2) {
            let c0 = self.decoder_map.get(&chars[i])
                .ok_or_else(|| format!("Invalid character in control: {}", chars[i]))?;
            let c1 = self.decoder_map.get(&chars[i + 1])
                .ok_or_else(|| format!("Invalid character in control: {}", chars[i + 1]))?;

            let d = c0 | (c1 << 6);

            // ツモ部分 (下位7ビット)
            let tsumo_data = d & 0x7F;
            let tsumo_axis_id = tsumo_data / 5;
            let tsumo_child_id = tsumo_data % 5;

            let axis_color = Self::tsumo_id_to_color(tsumo_axis_id);
            let child_color = Self::tsumo_id_to_color(tsumo_child_id);
            seq.push(Kumipuyo::new(axis_color, child_color));

            // 操作部分 (上位ビット)
            let h = d >> 7;
            let axis_x = h >> 2;
            let rot = h & 0x3;
            decisions.push(Decision::new(axis_x, rot));
        }

        Ok((seq, decisions))
    }

    fn tsumo_id_to_color(id: usize) -> PuyoColor {
        match id {
            0 => PuyoColor::RED,
            1 => PuyoColor::GREEN,
            2 => PuyoColor::BLUE,
            3 => PuyoColor::YELLOW,
            _ => PuyoColor::EMPTY,
        }
    }

    fn field_id_to_color(id: usize) -> PuyoColor {
        match id {
            0 => PuyoColor::EMPTY,
            1 => PuyoColor::RED,
            2 => PuyoColor::GREEN,
            3 => PuyoColor::BLUE,
            4 => PuyoColor::YELLOW,
            6 => PuyoColor::OJAMA,
            _ => PuyoColor::EMPTY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_field_only() {
        let decoder = PuyopDecoder::new();

        // テストケース: "420Aa9r9hj"
        // 期待される盤面:
        // .....Y
        // .G..YY
        // RGRRBB
        // RRGRGB
        let (field, seq, decisions) = decoder.decode_url("http://www.puyop.com/s/420Aa9r9hj").unwrap();

        assert_eq!(seq.len(), 0);
        assert_eq!(decisions.len(), 0);

        // 盤面の一部を検証
        assert_eq!(field.color(1, 1), PuyoColor::RED);
        assert_eq!(field.color(6, 4), PuyoColor::YELLOW);
    }

    #[test]
    fn test_decode_with_control() {
        let decoder = PuyopDecoder::new();

        // コントロール付きURL（仮想例）
        let url = "http://www.puyop.com/s/_0a0b";
        let (field, seq, decisions) = decoder.decode_url(url).unwrap();

        assert!(seq.len() > 0);
        assert_eq!(seq.len(), decisions.len());
    }

    #[test]
    fn test_decode_encoder_chars() {
        let decoder = PuyopDecoder::new();

        // ENCODER配列の全文字がデコードできることを確認
        for (i, &c) in PuyopDecoder::ENCODER.iter().enumerate() {
            assert_eq!(*decoder.decoder_map.get(&c).unwrap(), i);
        }
    }
}
