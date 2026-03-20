class Foo

class Bar

class Baz

class Qux

let Foo to-Bar => Bar
let Bar to-Baz => Baz
let Baz to-Qux => Qux

def Foo to-Bar => new Bar
def Bar to-Baz => new Baz
def Baz to-Qux => new Qux

class Main

let class Main main => Qux
def class Main main =>
  (new Foo) & to-Bar & to-Baz & to-Qux
