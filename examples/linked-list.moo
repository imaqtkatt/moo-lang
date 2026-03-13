-- Node

class Node value: int next: ?Node

-- LinkedList

class LinkedList head: ?Node

let LinkedList empty? => bool
def LinkedList empty? => if let head then false else true

let LinkedList set-head: ?Node => void
def LinkedList set-head: node => head = node

let class LinkedList empty => LinkedList
def class LinkedList empty => new LinkedList head: null

let LinkedList add: int => LinkedList
def LinkedList add: x =>
  let new-head = new Node value: x next: head in
  new LinkedList head: new-head

let LinkedList add-mut: int => void
def LinkedList add-mut: x =>
  let new-head = new Node value: x next: head in
  self set-head: new-head

-- Main

class Main

let class Main main => LinkedList
def class Main main =>
  let list = LinkedList empty in
  list add-mut: 1,
       add-mut: 2,
       add-mut: 3;
  list
