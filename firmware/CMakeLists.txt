cmake_minimum_required(VERSION 3.16)
set(PROJECT_VER "8")

# Board is selected with -D CAM_BOARD=<name> (see boards/*.args); the board
# type string comes from that board's defaults file so the OTA binary name
# always matches CONFIG_BOARD_TYPE. Legacy single-config builds fall back to
# the generated sdkconfig.
if(CAM_BOARD)
    file(READ "${CMAKE_SOURCE_DIR}/sdkconfig.defaults.${CAM_BOARD}" SDKCONFIG_CONTENT)
elseif(EXISTS "${CMAKE_SOURCE_DIR}/sdkconfig")
    file(READ "${CMAKE_SOURCE_DIR}/sdkconfig" SDKCONFIG_CONTENT)
else()
    set(SDKCONFIG_CONTENT "")
endif()
string(REGEX MATCH "CONFIG_BOARD_TYPE=\"([^\"]+)\"" _ "${SDKCONFIG_CONTENT}")
set(BOARD_TYPE "${CMAKE_MATCH_1}")
if(NOT BOARD_TYPE)
    set(BOARD_TYPE "unknown")
endif()
string(REGEX REPLACE "[^a-zA-Z0-9_]" "_" BOARD_SAFE "${BOARD_TYPE}")

include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(${BOARD_SAFE}_v${PROJECT_VER})
