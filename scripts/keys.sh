#!/bin/bash

for i in {a..z}
do
	echo "XK_$i,"
done

for i in {0..9}
do
	echo "XK_$i,"
done

special=(Return Tab space comma period)
for i in ${special[@]}
do
	echo "XK_$i,"
done
