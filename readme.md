## Another reimplementation of git-hierarchy, in Rust

See
- https://github.com/MichalMaruska/git-hierarchy
- https://github.com/MichalMaruska/git-hierarchy-go


## Difference:

More parts might be done natively, instead of invoking `git'
More care is necessary to sync the state after such invocations.


## todo:
might try using git2 with "vendored-libgit2"


## learnt about Rust/git-rs:

2-step downcasting from a trait object:
* Any ... type-erasure?  as_any produces Any.... and that has vtable, which...
* from Any ....allows to downcast<>


OnceCell

