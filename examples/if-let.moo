-- LuckyNumber

class LuckyNumber n: ?int

let LuckyNumber get-or: int => int
def LuckyNumber get-or: x =>
  if let n
    then n
    else x

-- Main

class Main

let class Main main => int
def class Main main =>
  let lucky = new LuckyNumber n: null in
  lucky get-or: 42
