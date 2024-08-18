#!/bin/bash

case $# in
    0) handlers="togglebar focusstack incnmaster setmfact zoom view killclient
setlayout togglefloating tag focusmon tagmon toggleview quit" ;;
    *) handlers=$* ;;
esac

for i in $handlers
do
    echo "pub(crate) unsafe extern \"C\" fn $i(arg: *const Arg) { unsafe { bindgen::$i(arg) } }"
    echo
done
