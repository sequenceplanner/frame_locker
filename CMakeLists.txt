cmake_minimum_required(VERSION 3.5)
project(frame_locker)

find_package(ament_cmake REQUIRED)

if(NOT DEFINED CMAKE_SUPPRESS_DEVELOPER_WARNINGS)
     set(CMAKE_SUPPRESS_DEVELOPER_WARNINGS 1 CACHE INTERNAL "No dev warnings")
endif()

include(r2r_cargo.cmake)

# put ros package dependencies here.
r2r_cargo(rcl                       # mandatory
          rcl_action                # mandatory
          rmw_fastrtps_cpp          # (needed to build with RMW_IMPLEMENTATION=rmw_fastrtps_cpp)
          FastRTPS                  # (needed to build with RMW_IMPLEMENTATION=rmw_fastrtps_cpp)
          tf2_msgs                  # to publish TF messages to tf and tf_static
          std_msgs                  # the Header message is here
          std_srvs                  # trigger service
          geometry_msgs             # the Transform and TransformStamped messages are here
          scene_manipulation_msgs   # the TransformLookup and ManipulateScene service msgs
         )

# install binaries
install(PROGRAMS
  ${CMAKE_SOURCE_DIR}/target/release/${PROJECT_NAME}
  DESTINATION lib/${PROJECT_NAME}
)

# we need this for ros/colcon
ament_package()
