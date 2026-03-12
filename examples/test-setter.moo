class Box inner: int

let Box set-inner: int => void
def Box set-inner: new-inner => inner = new-inner

let Box inner => int
def Box inner => inner



class Main

let class Main main => int
def class Main main =>
  let box = new Box inner: 40 in
  box set-inner: 41;
  box set-inner: 42;
  box inner
