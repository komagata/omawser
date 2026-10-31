# Sourced by the launcher; explicit settings always belong to the caller.
# Arguments are detected session type (yes/no for Wayland) and PCI GPU vendors.
oma_graphics_compat() {
  [ "${WEBKIT_DISABLE_DMABUF_RENDERER+x}" != x ] || return 0
  [ "$1" = yes ] || return 0
  case " $2 " in
    *0x10de*) export WEBKIT_DISABLE_DMABUF_RENDERER=1 ;;
  esac
}
