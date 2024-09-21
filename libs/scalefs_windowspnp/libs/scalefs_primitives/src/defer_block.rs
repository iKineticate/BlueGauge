// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

use std::mem::ManuallyDrop;

/* NOTES:
  - The defer! macro is inspired by the Swift language's defer statement.

  - Use the defer! macro to specify a block of code (usually a block of cleanup code) which should execute after other code which follows it in the same scope
    - A close (but not exact) language usage analogy is cleanup code in a C# finally block (in a try...finally sequence)
    - This macro has not been tested with panic scenarios; it is likely that the deferred block expression will only execute when its scope is exited normally

  - In determining how to implement a defer! macro, research focused primarily on how to get Rust to not drop a variable until its enclosing scope is been exited--even if the variable itself is not referenced further in the code
    - As demonstrated in testing, a variable wrapped with ManuallyDrop is not dropped until its parent object's instance is dropped; the parent struct's code can then implement the Drop trait which manually executes a defer block (which block is stored in the ManuallyDrop wrapper)
    - In tests, a ManuallyDrop-wrapped block expression's parent was dropped by Rust at the point that the parent's scope was exited; note that Rust's documentation on this behavior are thin and no guarantees were found that this behavior will continue in future versions of the Rust compiler
    - The parent object must have a defined variable name; using a "let _ =" construct with the creation of the parent instance resulted in the immediate dropping of that parent instance; it seems to be the capture of the parent instance which protects the ManuallyDrop-wrapped block
       expression until scope is exited

  - Summary of how the defer! macro and its DeferBlock instance operate:
    - DeferBlock is a struct which accepts a user-supplied block expression as its sole input; this block expression is stored using Rust's ManuallyDrop struct to prevent both the block expression and the DeferBlock itself from being dropped until the current scope is exited (as tested, see above)
    - The defer! macro creates an instance of DeferBlock to hold the supplied "defer" block expression
    - To provide guarantees around only executing once, DeferBlock requires that the supplied block expression be compatible with the FnOnce trait; this may require some creative solutions from consumers who need to mutate variables supplied in the block to the defer! macro
    - DeferBlock implements the Drop trait; this executes the user-supplied block expression when the DeferBlock goes out of scope.  This drop of the parent DeferBlock should happen no earlier than the time of exiting a scope (as testing, see above)

  - defer! behavior (as tested with Rust 2021 edition, compiled with Rust v1.74.1)
    - NOTE: defer! blocks should ideally be enclosed within an explicit scope
    - a single defer! block will execute after its scope is exited
        {
            defer! { 
                defer_expressions; // executed after its scope is exited
            } 
           
            other_expressions; // executed first
        }
    - two defer! blocks in sequence within the same scope are executed in reverse order (LIFO)
        {
            defer! { 
                defer_expressions; // executed third, after its (shared scope) is exited
            } 
           
            other_expressions; // executed first

            defer! { 
                defer_expressions; // executed second, after its (shared scope) is exited
            }
        }      
    - a defer! block in a scope which is more inner than another defer! block will be executed first
        { 
            defer! { 
                defer_expressions; // executed fourth, after its (outer) scope is exited
            } 
           
            other_expressions; // executed first

            { 
                defer! { 
                   defer_expressions; // executed third, after its (inner) scope is exited
                }

                other_expressions; // executed second
            }
        }
    - a defer! block contained within another defer! block will execute sequentially; this is not an anticipated scenario, but it is something to be aware of
        { 
            defer! { 
                defer_expressions; // executed second, after its scope is exited

                defer! { 
                   defer_expressions; // executed third, after its containing defer! is dropped
                }
            } 
           
            other_expressions; // executed first
        }
 */

 
 /* macro to define a defer block (and capture the user-supplied 'defer' block expression) */
 //
 // NOTE: we capture the block expression to defer using macro fragment-specifier "tt"; this is a Rust TokenTree (i.e. a single token or a token within matching delimiters), so we capture the full user-supplied block expression on which execution should be deferred until scope exit
 //       see: https://doc.rust-lang.org/reference/macros-by-example.html
 // NOTE: macros generally accept arguments rather than entire block expressions, so some research of forums and documentation plus experimentation was required to arrive at the the "( $($block_expression:tt)* )" parameter capture pattern (which matches the provided { } block) and the corresponding
 //       "$($block_expression)*" parameter usage pattern.  There may be room for additional improvement here, and there may also be some expressions which are not compatible.
 #[macro_export]
 macro_rules! defer {
     ( $($block_expression:tt)* ) => {
         // NOTE: we need to store the DeferBlock in a variable (i.e. not "let _ =") so that it doesn't get dropped immediately.
         // NOTE: we use a name here starting with a double-underscore just in case the variable is ever available in the user's code (e.g. in their current scope or in their deferred block expression); a single underscore is a common usage pattern so we prefix all macro variables
         //       with double-underscore
         // NOTE: the variable we create here should be local to this defer block--and it should not be affected by other defer! macro instantiations (even ones in a more-inner scope)--so its name should really be irrelevant generally (and especially to code outside the macro)
         //
         // create a DeferBlock (which contains a field of type ManullyDrop<T>); in our testing, this variable is then dropped by Rust when the scope that encloses the defer! macro instantiation is exited
         let __defer_block = scalefs_primitives::defer_block::DeferBlock::new(|| { $($block_expression)* });
     };
 }
 
 
 /* implementation of a defer block */
 //
 // NOTE: we use a one-time function to ensure that we do not (and cannot) call the deferred block expression twice
 pub struct DeferBlock<T> where T: FnOnce() {
     // NOTE: we use the ManuallyDrop wrapper with the deferred block expression to prevent the DeferBlock from being dropped until after the DeferBlock's (defer! macro's) scope is executed (and after the block expression is executed)
     deferred_block_expression: ManuallyDrop<T>,
 }
 //
 impl<T> DeferBlock<T> where T: FnOnce() {
     // NOTE: for maximum flexibility, we'll let Rust infer the type of block expression; if this is problematic, we may want to consider requiring an empty args list and an empty (unit type) result.
     // pub fn new(block_expression: fn() -> ()) -> Self {        
     pub fn new(block_expression: T) -> Self {
         DeferBlock {
             deferred_block_expression: ManuallyDrop::new(block_expression)
         }
     }
 }
 
 // NOTE: DeferBlock implements the Drop trait so that it can execute the user-provided "defer" block expression when the DeferBlock itself goes out of scope
 impl<T> Drop for DeferBlock<T> where T: FnOnce() {
     fn drop(&mut self) {
         /* capture a reference to the stored deferred_block_expression (so that we can execute it) */

         // NOTE: there are at least two methods to obtain a mutable reference to our struct's deferred block expression (below)
 
         // option 1: a reasonable (yet "unsafe") way to capture the reference; we must do this with caution, but it is a fairly reasonable approach
         //
         // move the deferred block expression out of our struct instance (so that we can execute it)
         // NOTE: this operation is called via unsafe as it technically leaves the state of the container unchanged (i.e. it only semantically moves the contained value out of the struct); we must be extremely
         //       careful not to use this ManualDrop instance again (other than dropping its contents)
         let block_expression = unsafe { ManuallyDrop::take(&mut self.deferred_block_expression) };
 
         // option 2: manually capture a reference using the std::ptr::read hack (defer then re-ref the pointer and simply create a second pointer)
         //
         // capture a reference to the deferred block expression without moving it; this is just as unsafe as or even more unsafe than option 1
         // let block_expression = unsafe { std::ptr::read(&*self.deferred_block_expression) };
                 
         /* execute the deferred block expression via the reference we just captured */
         (block_expression)();
 
         /* out of an abundance of caution, drop the contents of deferred_block_expression */
         // NOTE: this may technically be unnecessary, but it should be safe to do after capturing the reference using either of the two reference-capturing options above
         unsafe { ManuallyDrop::drop(&mut self.deferred_block_expression); }
     }
 }