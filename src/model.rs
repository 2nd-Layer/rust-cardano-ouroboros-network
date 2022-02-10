use crate::Error;

#[derive(Debug, Clone)]
pub struct Point {
    pub slot: i64,
    pub hash: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Tip {
    pub block_number: i64,
    pub slot_number: i64,
    pub hash: Vec<u8>,
}

impl Into<Point> for Tip {
    fn into(self) -> Point {
        Point {
            slot: self.slot_number,
            hash: self.hash,
        }
    }
}

impl TryFrom<(i64, &str)> for Point {
    type Error = Error;

    fn try_from(pair: (i64, &str)) -> Result<Point, Self::Error> {
        let (slot, hash) = pair;
        Ok(Point {
            slot,
            hash: hex::decode(hash).map_err(|_| "Bad hash hex.".to_string())?,
        })
    }
}
