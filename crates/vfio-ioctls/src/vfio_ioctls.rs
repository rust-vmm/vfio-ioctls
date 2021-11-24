// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
//
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::ffi::CStr;
use std::fs::File;
use std::mem::size_of;
use std::os::unix::io::AsRawFd;

use vfio_bindings::bindings::vfio::*;
use vmm_sys_util::errno::Error as SysError;

use crate::vfio_device::vfio_region_info_with_cap;
use crate::{Result, VfioContainer, VfioDevice, VfioError};

ioctl_io_nr!(VFIO_GET_API_VERSION, VFIO_TYPE, VFIO_BASE);
ioctl_io_nr!(VFIO_CHECK_EXTENSION, VFIO_TYPE, VFIO_BASE + 1);
ioctl_io_nr!(VFIO_SET_IOMMU, VFIO_TYPE, VFIO_BASE + 2);
ioctl_io_nr!(VFIO_GROUP_GET_STATUS, VFIO_TYPE, VFIO_BASE + 3);
ioctl_io_nr!(VFIO_GROUP_SET_CONTAINER, VFIO_TYPE, VFIO_BASE + 4);
ioctl_io_nr!(VFIO_GROUP_UNSET_CONTAINER, VFIO_TYPE, VFIO_BASE + 5);
ioctl_io_nr!(VFIO_GROUP_GET_DEVICE_FD, VFIO_TYPE, VFIO_BASE + 6);
ioctl_io_nr!(VFIO_DEVICE_GET_INFO, VFIO_TYPE, VFIO_BASE + 7);
ioctl_io_nr!(VFIO_DEVICE_GET_REGION_INFO, VFIO_TYPE, VFIO_BASE + 8);
ioctl_io_nr!(VFIO_DEVICE_GET_IRQ_INFO, VFIO_TYPE, VFIO_BASE + 9);
ioctl_io_nr!(VFIO_DEVICE_SET_IRQS, VFIO_TYPE, VFIO_BASE + 10);
ioctl_io_nr!(VFIO_DEVICE_RESET, VFIO_TYPE, VFIO_BASE + 11);
ioctl_io_nr!(
    VFIO_DEVICE_GET_PCI_HOT_RESET_INFO,
    VFIO_TYPE,
    VFIO_BASE + 12
);
ioctl_io_nr!(VFIO_DEVICE_PCI_HOT_RESET, VFIO_TYPE, VFIO_BASE + 13);
ioctl_io_nr!(VFIO_DEVICE_QUERY_GFX_PLANE, VFIO_TYPE, VFIO_BASE + 14);
ioctl_io_nr!(VFIO_DEVICE_GET_GFX_DMABUF, VFIO_TYPE, VFIO_BASE + 15);
ioctl_io_nr!(VFIO_DEVICE_IOEVENTFD, VFIO_TYPE, VFIO_BASE + 16);
ioctl_io_nr!(VFIO_IOMMU_GET_INFO, VFIO_TYPE, VFIO_BASE + 12);
ioctl_io_nr!(VFIO_IOMMU_MAP_DMA, VFIO_TYPE, VFIO_BASE + 13);
ioctl_io_nr!(VFIO_IOMMU_UNMAP_DMA, VFIO_TYPE, VFIO_BASE + 14);
ioctl_io_nr!(VFIO_IOMMU_ENABLE, VFIO_TYPE, VFIO_BASE + 15);
ioctl_io_nr!(VFIO_IOMMU_DISABLE, VFIO_TYPE, VFIO_BASE + 16);

// Safety:
// - absolutely trust the underlying kernel
// - absolutely trust data returned by the underlying kernel
// - assume kernel will return error if caller passes in invalid file handle, parameter or buffer.
pub(crate) mod vfio_syscall {
    use std::os::unix::io::FromRawFd;
    use vmm_sys_util::ioctl::{
        ioctl, ioctl_with_mut_ref, ioctl_with_ptr, ioctl_with_ref, ioctl_with_val,
    };
    use super::*;
    use crate::vfio_device::VfioDeviceInfo;
    use crate::VfioGroup;

    pub(crate) fn check_api_version(container: &VfioContainer) -> i32 {
        // Safe as file is vfio container fd and ioctl is defined by kernel.
        unsafe { ioctl(container, VFIO_GET_API_VERSION()) }
    }

    pub(crate) fn check_extension(container: &VfioContainer, val: u32) -> Result<u32> {
        // Safe as file is vfio container and make sure val is valid.
        let ret = unsafe { ioctl_with_val(container, VFIO_CHECK_EXTENSION(), val.into()) };
        if ret < 0 {
            Err(VfioError::VfioExtension)
        } else {
            Ok(ret as u32)
        }
    }

    pub(crate) fn set_iommu(container: &VfioContainer, val: u32) -> Result<()> {
        // Safe as file is vfio container and make sure val is valid.
        let ret = unsafe { ioctl_with_val(container, VFIO_SET_IOMMU(), val.into()) };
        if ret < 0 {
            Err(VfioError::ContainerSetIOMMU)
        } else {
            Ok(())
        }
    }

    pub(crate) fn map_dma(
        container: &VfioContainer,
        dma_map: &vfio_iommu_type1_dma_map,
    ) -> Result<()> {
        // Safe as file is vfio container, dma_map is constructed by us, and
        // we check the return value
        let ret = unsafe { ioctl_with_ref(container, VFIO_IOMMU_MAP_DMA(), dma_map) };
        if ret != 0 {
            Err(VfioError::IommuDmaMap)
        } else {
            Ok(())
        }
    }

    pub(crate) fn unmap_dma(
        container: &VfioContainer,
        dma_map: &mut vfio_iommu_type1_dma_unmap,
    ) -> Result<()> {
        // Safe as file is vfio container, dma_unmap is constructed by us, and
        // we check the return value
        let ret = unsafe { ioctl_with_ref(container, VFIO_IOMMU_UNMAP_DMA(), dma_map) };
        if ret != 0 {
            Err(VfioError::IommuDmaUnmap)
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_group_status(
        file: &File,
        group_status: &mut vfio_group_status,
    ) -> Result<()> {
        // Safe as we are the owner of group and group_status which are valid value.
        let ret = unsafe { ioctl_with_mut_ref(file, VFIO_GROUP_GET_STATUS(), group_status) };
        if ret < 0 {
            Err(VfioError::GetGroupStatus)
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_group_device_fd(group: &VfioGroup, path: &CStr) -> Result<File> {
        // Safe as we are the owner of self and path_ptr which are valid value.
        let fd = unsafe { ioctl_with_ptr(group, VFIO_GROUP_GET_DEVICE_FD(), path.as_ptr()) };
        if fd < 0 {
            Err(VfioError::GroupGetDeviceFD)
        } else {
            // Safe as fd is valid FD
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }

    pub(crate) fn set_group_container(group: &VfioGroup, container: &VfioContainer) -> Result<()> {
        let container_raw_fd = container.as_raw_fd();
        // Safe as we are the owner of group and container_raw_fd which are valid value,
        // and we verify the ret value
        let ret = unsafe { ioctl_with_ref(group, VFIO_GROUP_SET_CONTAINER(), &container_raw_fd) };
        if ret < 0 {
            Err(VfioError::GroupSetContainer)
        } else {
            Ok(())
        }
    }

    pub(crate) fn unset_group_container(
        group: &VfioGroup,
        container: &VfioContainer,
    ) -> Result<()> {
        let container_raw_fd = container.as_raw_fd();
        // Safe as we are the owner of self and container_raw_fd which are valid value.
        let ret = unsafe { ioctl_with_ref(group, VFIO_GROUP_UNSET_CONTAINER(), &container_raw_fd) };
        if ret < 0 {
            Err(VfioError::GroupSetContainer)
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_device_info(file: &File, dev_info: &mut vfio_device_info) -> Result<()> {
        // Safe as we are the owner of dev and dev_info which are valid value,
        // and we verify the return value.
        let ret = unsafe { ioctl_with_mut_ref(file, VFIO_DEVICE_GET_INFO(), dev_info) };
        if ret < 0 {
            Err(VfioError::VfioDeviceGetInfo)
        } else {
            Ok(())
        }
    }

    pub(crate) fn set_device_irqs(device: &VfioDevice, irq_set: &[vfio_irq_set]) -> Result<()> {
        if irq_set.is_empty()
            || irq_set[0].argsz as usize > irq_set.len() * size_of::<vfio_irq_set>()
        {
            Err(VfioError::VfioDeviceSetIrq)
        } else {
            // Safe as we are the owner of self and irq_set which are valid value
            let ret = unsafe { ioctl_with_ref(device, VFIO_DEVICE_SET_IRQS(), &irq_set[0]) };
            if ret < 0 {
                Err(VfioError::VfioDeviceSetIrq)
            } else {
                Ok(())
            }
        }
    }

    pub(crate) fn reset(device: &VfioDevice) -> i32 {
        // Safe as file is vfio device
        unsafe { ioctl(device, VFIO_DEVICE_RESET()) }
    }

    pub(crate) fn get_device_irq_info(
        dev_info: &VfioDeviceInfo,
        irq_info: &mut vfio_irq_info,
    ) -> Result<()> {
        // Safe as we are the owner of dev and irq_info which are valid value
        let ret = unsafe { ioctl_with_mut_ref(dev_info, VFIO_DEVICE_GET_IRQ_INFO(), irq_info) };
        if ret < 0 {
            Err(VfioError::VfioDeviceGetRegionInfo(SysError::new(-ret)))
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_device_region_info(
        dev_info: &VfioDeviceInfo,
        reg_info: &mut vfio_region_info,
    ) -> Result<()> {
        // Safe as we are the owner of dev and region_info which are valid value
        // and we verify the return value.
        let ret = unsafe { ioctl_with_mut_ref(dev_info, VFIO_DEVICE_GET_REGION_INFO(), reg_info) };
        if ret < 0 {
            Err(VfioError::VfioDeviceGetRegionInfo(SysError::new(-ret)))
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_device_region_info_cap(
        dev_info: &VfioDeviceInfo,
        reg_infos: &mut [vfio_region_info_with_cap],
    ) -> Result<()> {
        if reg_infos.is_empty()
            || reg_infos[0].region_info.argsz as usize
                > reg_infos.len() * size_of::<vfio_region_info>()
        {
            Err(VfioError::VfioDeviceGetRegionInfo(SysError::new(
                libc::EINVAL,
            )))
        } else {
            // Safe as we are the owner of dev and region_info which are valid value,
            // and we verify the return value.
            let ret = unsafe {
                ioctl_with_mut_ref(dev_info, VFIO_DEVICE_GET_REGION_INFO(), &mut reg_infos[0])
            };
            if ret < 0 {
                Err(VfioError::VfioDeviceGetRegionInfo(SysError::new(-ret)))
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vfio_ioctl_code() {
        assert_eq!(VFIO_GET_API_VERSION(), 15204);
        assert_eq!(VFIO_CHECK_EXTENSION(), 15205);
        assert_eq!(VFIO_SET_IOMMU(), 15206);
        assert_eq!(VFIO_GROUP_GET_STATUS(), 15207);
        assert_eq!(VFIO_GROUP_SET_CONTAINER(), 15208);
        assert_eq!(VFIO_GROUP_UNSET_CONTAINER(), 15209);
        assert_eq!(VFIO_GROUP_GET_DEVICE_FD(), 15210);
        assert_eq!(VFIO_DEVICE_GET_INFO(), 15211);
        assert_eq!(VFIO_DEVICE_GET_REGION_INFO(), 15212);
        assert_eq!(VFIO_DEVICE_GET_IRQ_INFO(), 15213);
        assert_eq!(VFIO_DEVICE_SET_IRQS(), 15214);
        assert_eq!(VFIO_DEVICE_RESET(), 15215);
        assert_eq!(VFIO_DEVICE_IOEVENTFD(), 15220);
        assert_eq!(VFIO_IOMMU_DISABLE(), 15220);
    }
}
