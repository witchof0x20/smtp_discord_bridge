# Copyright 2020 Jade
# This file is part of smtp_discord_bridge.

# smtp_discord_bridge is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.

# smtp_discord_bridge is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.

# You should have received a copy of the GNU General Public License
# along with smtp_discord_bridge.  If not, see <https:#www.gnu.org/licenses/>.
{ pkgs ? import <nixpkgs> { }}:
pkgs.mkShell {
  buildInputs = with pkgs; [
    # TODO: add in specific rust version from the rust overlay 
    rustup
    # For serenity
    pkgconfig
    openssl
  ];
}

