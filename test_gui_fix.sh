#!/bin/bash

echo "Testing as-gui terminal restoration fix..."
echo "=========================================="
echo
echo "Instructions:"
echo "1. Run the as-gui application: ./target/release/as-gui"
echo "2. Navigate to any package and press Enter to install it"
echo "3. Cancel the installation (Ctrl+C when sudo asks for password)"
echo "4. Press Enter to return to GUI"
echo "5. Check if the GUI appears correctly or disappears"
echo
echo "If the GUI appears correctly after returning from terminal mode,"
echo "then the fix is working!"
echo
echo "Press Enter to start as-gui..."
read

cd /home/mark/.local/bin/as-gui
./target/release/as-gui
