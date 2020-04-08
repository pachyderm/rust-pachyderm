#!/bin/bash

# Remove gogoproto annotations, as otherwise the build will fail. It's
# done here instead of in `build.rs`, because `build.rs` cannot write
# outside of the `./target` directory.
for i in $(find ./proto -name "*.proto"); do
    sed -i s/import.*gogo.proto.*\;// $i
    sed -i 's/\[.*gogoproto.*\]//' $i
done
