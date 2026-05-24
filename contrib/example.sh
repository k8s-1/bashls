#!/bin/bash

# SC2086: unquoted variable
greet() {
  echo Hello $1
}

# SC2046: unquoted command substitution
files=$(ls /tmp)
rm $files

# SC2196: egrep is deprecated
egrep "foo" /etc/hosts

# SC2006: backtick command substitution
result=`date`

greet "world"
echo $result
