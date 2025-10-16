wit_bindgen::generate!();

struct CheckReactBoundary;

impl Guest for CheckReactBoundary {
    fn check() -> () {
        todo!()
    }
}

export!(CheckReactBoundary);
