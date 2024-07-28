#!/bin/bash

handlers="buttonpress clientmessage configurerequest configurenotify
destroynotify enternotify expose focusin keypress mappingnotify maprequest
motionnotify propertynotify unmapnotify"

for i in $handlers
do
    echo "fn $i(e: *mut XEvent) { unsafe { bindgen::$i(e) } }"
    echo
done
