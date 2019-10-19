use crate::grammar::{Expression, ExpressionTok::{EValue, EBinaryOp}, Value::{self, EConstant}, Constant,    BinaryOp::{self, EPlus, EMinus, EMul, EDiv, EMod, EExp, ELT, ELTE, EEQ, ENE, EGTE, EGT, EOR, EAND}};
use crate::error::Error;



// Vec seems really inefficient to me because remove() does not just increment the internal pointer -- it shifts data all around.  There's also split_* methods but they seem to be designed to return new Vecs, not modify self.
// Just use slices instead, which I know will be very efficient:
fn read(bs:&mut &[u8]) -> Result<u8, Error> {
    if bs.len() > 0 {
        let b = bs[0];
        *bs = &bs[1..];
        Ok(b)
    } else { Err(Error::new("EOF")) }
}
fn peek(bs:&[u8], skip:usize) -> Option<u8> {
    if bs.len() > skip { Some(bs[skip])
    } else { None }
}
fn is_at_eof(bs:&[u8]) -> bool { bs.len() == 0 }
fn peek_is(bs:&[u8], skip:usize, val:u8) -> bool {
    match peek(bs,skip) {
        None => false,
        Some(b) => b==val,
    }
}

fn is_space(b:u8) -> bool {
    match b {
    b' ' | b'\t' | b'\r' | b'\n' => true,
    _ => false,
    }
}
fn space(bs:&mut &[u8]) {
    while let Some(b) = peek(bs,0) {
        if !is_space(b) { break }
        let _ = read(bs);
    }
}



struct Parser<'a> {
    is_const_byte:Option<&'a dyn Fn(u8,usize)->bool>,
    is_func_byte :Option<&'a dyn Fn(u8,usize)->bool>,
    is_var_byte  :Option<&'a dyn Fn(u8,usize)->bool>,
}

impl<'a> Parser<'a> {
    fn default_is_const_byte(b:u8, i:usize) -> bool {
        if b'0'<=b && b<=b'9' || b==b'.' { return true }
        if i>0 && ( b==b'k' || b==b'K' || b==b'M' || b==b'G' || b==b'T' ) { return true }
        return false
    }
    fn default_is_var_byte(b:u8, i:usize) -> bool {
        (b'A'<=b && b<=b'Z') || (b'a'<=b && b<=b'z') || b==b'_' || (i>0 && b'0'<=b && b<=b'9')
    }

    fn call_is_const_byte(&self, bo:Option<u8>, i:usize) -> bool {
        match bo {
            Some(b) => match self.is_const_byte {
                Some(f) => f(b,i),
                None => Parser::default_is_const_byte(b,i),
            }
            None => false
        }
    }
    fn call_is_func_byte(&self, bo:Option<u8>, i:usize) -> bool {
        match bo {
            Some(b) => match self.is_func_byte {
                Some(f) => f(b,i),
                None => Parser::default_is_var_byte(b,i),
            }
            None => false
        }
    }
    fn call_is_var_byte(&self, bo:Option<u8>, i:usize) -> bool {
        match bo {
            Some(b) => match self.is_var_byte {
                Some(f) => f(b,i),
                None => Parser::default_is_var_byte(b,i),
            }
            None => false
        }
    }

    pub fn parse(&self, s:&str) -> Result<Expression, Error> {
        let bs = &mut s.as_bytes();
        self.read_expression(bs, true)
    }

    fn read_expression(&self, bs:&mut &[u8], expect_eof:bool) -> Result<Expression, Error> {
        let val = self.read_value(bs).map_err(|e| e.pre("read_value"))?;
        let mut vec = vec![EValue(val)];
        while self.peek_binaryop(bs) {
            let bop = self.read_binaryop(bs).map_err(|e| e.pre("read_binaryop"))?;
            let val = self.read_value(bs).map_err(|e| e.pre("read_value"))?;
            vec.push(EBinaryOp(bop)); vec.push(EValue(val));
        }
        space(bs);
        if expect_eof && !is_at_eof(bs) { return Err(Error::new("unparsed tokens remaining")); }
        Ok(Expression(vec.into_boxed_slice()))
    }

    fn read_value(&self, bs:&mut &[u8]) -> Result<Value, Error> {
        if self.peek_const(bs) {
            return match self.read_const(bs) {
                Ok(constant) => Ok(EConstant(constant)),
                Err(err) => Err(err),
            }
        }
        //if self.peek_unaryop(bs) { return self.read_unaryop(bs) }
        //if self.peek_callable(bs) { return self.read_callable(bs) }
        //if self.peek_var(bs) { return self.read_var(bs) }
        Err(Error::new("InvalidValue"))
    }

    fn peek_const(&self, bs:&mut &[u8]) -> bool {
        space(bs);
        self.call_is_const_byte(peek(bs,0),0)
    }
    fn read_const(&self, bs:&mut &[u8]) -> Result<Constant, Error> {
        space(bs);
        let mut buf : Vec<u8> = Vec::with_capacity(16);

        loop {
            {
                let c = peek(bs,0);
                let r = self.call_is_const_byte(c,buf.len());
                if !r { break }
            }

            buf.push(read(bs)?);
        }

        let mut multiple = 1.0;
        if buf.len()>0 {
            match buf.last().unwrap() {
                b'k' | b'K' => {   multiple=1000.0; buf.pop(); }
                b'M' => {       multiple=1000000.0; buf.pop(); }
                b'G' => {    multiple=1000000000.0; buf.pop(); }
                b'T' => { multiple=1000000000000.0; buf.pop(); }
                _ => {}
            }
        }

        let bufstr = std::str::from_utf8(buf.as_slice()).map_err(|_| Error::new("Utf8Error"))?;
        let val = bufstr.parse::<f64>().map_err(|_| {
            Error::new("parse<f64> error").pre(bufstr)
        })?;
        Ok(Constant(val*multiple))
    }




    

    fn peek_binaryop(&self, bs:&mut &[u8]) -> bool {
        space(bs);
        match peek(bs,0) {
            None => false,
            Some(b) => match b {
                b'+'|b'-'|b'*'|b'/'|b'%'|b'^'|b'<'|b'>' => true,
                b'=' => peek_is(bs,1,b'='),
                b'!' => peek_is(bs,1,b'='),
                b'o' => peek_is(bs,1,b'r'),
                b'a' => peek_is(bs,1,b'n') && peek_is(bs,2,b'd'),
                _ => false,
            }
        }
    }
    fn read_binaryop(&self, bs:&mut &[u8]) -> Result<BinaryOp, Error> {
        let err = Error::new("illegal binaryop");
        space(bs);
        match read(bs)? {
            b'+' => Ok(EPlus),
            b'-' => Ok(EMinus),
            b'*' => Ok(EMul),
            b'/' => Ok(EDiv),
            b'%' => Ok(EMod),
            b'^' => Ok(EExp),
            b'<' => if peek_is(bs,0,b'=') { read(bs)?; Ok(ELTE)
                    } else { Ok(ELT) },
            b'>' => if peek_is(bs,0,b'=') { read(bs)?; Ok(EGTE)
                    } else { Ok(EGT) },
            b'=' => if peek_is(bs,0,b'=') { read(bs)?; Ok(EEQ)
                    } else { Err(err) },
            b'!' => if peek_is(bs,0,b'=') { read(bs)?; Ok(ENE)
                    } else { Err(err) },
            b'o' => if peek_is(bs,0,b'r') { read(bs)?; Ok(EOR)
                    } else { Err(err) },
            b'a' => if peek_is(bs,0,b'n') && peek_is(bs,1,b'd') { read(bs)?; read(bs)?; Ok(EAND)
                    } else { Err(err) },
            _ => Err(err),
        }
    }
}

//---- Tests:

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn util() {
        match (|| -> Result<(),Error> {
            let bsarr = [1,2,3];
            let bs = &mut &bsarr[..];

            assert_eq!(peek(bs,0), Some(1));
            assert_eq!(peek(bs,1), Some(2));
            assert_eq!(peek(bs,2), Some(3));
            assert_eq!(peek(bs,3), None);

            assert_eq!(read(bs)?, 1);
            assert_eq!(read(bs)?, 2);
            assert_eq!(read(bs)?, 3);
            match read(bs).err() {
                Some(Error{..}) => {}  // Can I improve this so I can match the "EOF" ?
                None => panic!("I expected an EOF")
            }

            Ok(())
        })() {
            Ok(_) => {}
            Err(_) => {
                unimplemented!();
            }
        }

        assert!(is_at_eof(&[]));
        assert!(!is_at_eof(&[1]));
        assert!(is_at_eof(b""));
        assert!(!is_at_eof(b"x"));

        assert!(is_space(b' '));
        assert!(is_space(b'\t'));
        assert!(is_space(b'\r'));
        assert!(is_space(b'\n'));
        assert!(!is_space(b'a'));
        assert!(!is_space(b'1'));
        assert!(!is_space(b'.'));

        {
            let bsarr = b"  abc 123   ";
            let bs = &mut &bsarr[..];
            space(bs);
            assert_eq!(bs, b"abc 123   ");
        }
    }

    #[test]
    fn parser() {
        let p = Parser{
            is_const_byte:None,
            is_func_byte:None,
            is_var_byte:None,
        };
        assert!(p.call_is_func_byte(Some(b'a'),0));
        assert!(p.call_is_var_byte(Some(b'a'),0));
        assert!(!p.call_is_const_byte(Some(b'a'),0));

        let p = Parser{
            is_const_byte:Some(&|_:u8, _:usize| true),
            is_func_byte:None,
            is_var_byte:None,
        };
        assert!(p.call_is_const_byte(Some(b'a'),0));

        let p = Parser{
            is_const_byte:None,
            is_func_byte:None,
            is_var_byte:None,
        };
        
        {
            let bsarr = b"12.34";
            let bs = &mut &bsarr[..];
            assert_eq!(p.read_value(bs), Ok(EConstant(Constant(12.34))));
        }

        assert_eq!(p.parse("12.34 + 43.21 + 11.11"),
                   Ok(Expression(Box::new([
                        EValue(EConstant(Constant(12.34))),
                        EBinaryOp(EPlus),
                        EValue(EConstant(Constant(43.21))),
                        EBinaryOp(EPlus),
                        EValue(EConstant(Constant(11.11)))]))));
    }
}

