// move_semantics1.rs
//
// Hints at the bottom.

#[test]
fn main() {
    let vec0 = vec![22, 44, 66];

    let vec1 = fill_vec(vec0);
    // fill_vec takes ownership over v0, therefore you cannot print it

    assert_eq!(vec1, vec![22, 44, 66, 88]);
}

fn fill_vec(vec: Vec<i32>) -> Vec<i32> {
    let mut vec = vec; // added mut keyword

    vec.push(88);
    assert_ne!(vec, vec![22, 44, 66]);
    
    vec
}






















// So you've got the "cannot borrow immutable local variable `vec` as mutable" error on the line
// where we push an element to the vector, right?
// The fix for this is going to be adding one keyword, and the addition is NOT on the line where
// we push to the vector (where the error is).

// Also: Try accessing `vec0` after having called `fill_vec()`. See what happens!