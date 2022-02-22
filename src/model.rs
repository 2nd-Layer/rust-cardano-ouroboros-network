use crate::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Point {
    pub slot: u64,
    pub hash: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tip {
    pub block_number: i64,
    pub slot_number: u64,
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

impl TryFrom<(u64, &str)> for Point {
    type Error = Error;

    fn try_from(pair: (u64, &str)) -> Result<Point, Self::Error> {
        let (slot, hash) = pair;
        Ok(Point {
            slot,
            hash: hex::decode(hash).map_err(|_| "Bad hash hex.".to_string())?,
        })
    }
}

impl From<(u64, &[u8])> for Point {
    fn from(pair: (u64, &[u8])) -> Point {
        let (slot, hash) = pair;
        Point { slot, hash: hash.to_vec() }
    }
}
