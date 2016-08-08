#!/bin/sh
for f in *.pasm
do
    echo "running pasm for $f"
    pasm -b "$f"
done

