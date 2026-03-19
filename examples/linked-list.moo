-- Node

class Node[T] value: T next: ?Node[T]

-- LinkedList

class LinkedList[T] head: ?Node[T]

let LinkedList empty? => bool
def LinkedList empty? => if let head then false else true

let LinkedList set-head: ?Node[T] => void
def LinkedList set-head: node => head = node

let class LinkedList empty => LinkedList[T]
def class LinkedList empty => new LinkedList[T] head: null

let LinkedList add: T => LinkedList[T]
def LinkedList add: x =>
  let new-head = new Node[T] value: x next: head in
  new LinkedList[T] head: new-head

let LinkedList add-mut: T => void
def LinkedList add-mut: x =>
  let new-head = new Node[T] value: x next: head in
  self set-head: new-head

-- Main

class Main

let class Main main => LinkedList[int]
def class Main main =>
  let list = new LinkedList[int] head: null in
  list add-mut: 1,
       add-mut: 2,
       add-mut: 3;
  list
