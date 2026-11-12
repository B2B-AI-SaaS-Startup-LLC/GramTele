//! Debug-only PrintDead<T>; Drop executes before dropping the wrapped value.
#[cfg(debug_assertions)]
pub struct PrintDead<T>(pub T);
#[cfg(debug_assertions)]
impl<T> Drop for PrintDead<T> {
    fn drop(&mut self) {
        println!(
            "\x1b[32mInstance of class {} is dead!\x1b[0m",
            std::any::type_name::<T>()
        );
    }
}
#[cfg(debug_assertions)]
impl<T> std::ops::Deref for PrintDead<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
#[cfg(debug_assertions)]
impl<T> std::ops::DerefMut for PrintDead<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
#[cfg(all(test, debug_assertions))]
mod tests {
    #[test]
    fn drops_wrapped_object() {
        let token = std::rc::Rc::new(());
        {
            let wrapped = super::PrintDead(token.clone());
            assert_eq!(std::rc::Rc::strong_count(&wrapped), 2);
        }
        assert_eq!(std::rc::Rc::strong_count(&token), 1);
    }
}
