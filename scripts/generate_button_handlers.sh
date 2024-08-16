#!/bin/bash

handlers="setlayout zoom spawn movemouse togglefloating resizemouse view
toggleview tag toggletag"

for i in $handlers
do
    echo "pub fn $i(arg: &Arg) { todo!() }"
    echo
done
