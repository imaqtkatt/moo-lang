-- Boolean

class Boolean inner: bool

let class Boolean bool: bool => Boolean
def class Boolean bool: b => new Boolean inner: b

let Boolean unwrap => bool
def Boolean unwrap => inner

let Boolean negate => Boolean
def Boolean negate =>
  if inner
    then Boolean bool: false
    else Boolean bool: true

let Boolean and: Boolean => Boolean
def Boolean and: next =>
  if inner
    then next
    else self

let Boolean or: Boolean => Boolean
def Boolean or: next =>
  if inner
    then self
    else next

-- Main

class Main

let class Main main => bool
def class Main main =>
  let p = Boolean bool: true in
  let q = Boolean bool: false in
  (p and: q) unwrap
