//! Integer minor-unit money. Never use floating point.

pub type Minor = i64;

pub fn add(a: Minor, b: Minor) -> AppMoneyResult {
    a.checked_add(b).ok_or(MoneyError::Overflow)
}

pub fn sub(a: Minor, b: Minor) -> AppMoneyResult {
    a.checked_sub(b).ok_or(MoneyError::Overflow)
}

pub fn change(tendered: Minor, due: Minor) -> Result<Minor, MoneyError> {
    if tendered < due {
        return Err(MoneyError::InsufficientTender);
    }
    sub(tendered, due)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoneyError {
    Overflow,
    InsufficientTender,
    Negative,
    MissingSnapshot,
}

pub type AppMoneyResult = Result<Minor, MoneyError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sub_change() {
        assert_eq!(add(5000, 2275).unwrap(), 7275);
        assert_eq!(sub(15000, 12275).unwrap(), 2725);
        assert_eq!(change(15000, 12275).unwrap(), 2725);
        assert_eq!(change(5000, 12275), Err(MoneyError::InsufficientTender));
    }

    #[test]
    fn no_float() {
        let coke = 2500_i64;
        let two = coke.checked_mul(2).unwrap();
        assert_eq!(two, 5000);
    }
}
