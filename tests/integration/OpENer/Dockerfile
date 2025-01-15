FROM gcc:14

RUN apt-get update

RUN apt-get install -y \
                    wget \
                    cmake \
                    libcap-dev \
                    doxygen

WORKDIR /tmp
ARG ROOT_REPO_PATH=/tmp/OpENer
ARG BUILD_PATH=${ROOT_REPO_PATH}/bin/posix

RUN git clone https://github.com/EIPStackGroup/OpENer.git \
    && cd ${ROOT_REPO_PATH} \
    && git reset --hard 70d6947c6ef97c5c8ccbc908b94651432e18cf9c \
    && cd ${BUILD_PATH} \
    && ./setup_posix.sh \
    && make

WORKDIR ${BUILD_PATH}/src/ports/POSIX

CMD ["./OpENer", "eth0"]